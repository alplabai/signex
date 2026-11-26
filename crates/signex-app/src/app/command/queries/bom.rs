//! `query_bom` (#2938) and `search_parts` (#1297): the priced BOM of a
//! project and the parametric part search, both from cached supplier data
//! only. `query_bom` builds the BOM off the UI thread as `bom_cost` and the
//! BOM export build it, and is answered once that lands; `search_parts`
//! reads the open libraries in memory and answers at once. Neither fetches
//! a price or changes anything. Shapes: `docs/CLI.md`, "BOM and part
//! search".

use iced::Task;
use serde_json::{Value, json};
use signex_output::bom::{BuildOrder, priced_bom};
use signex_types::project::is_base_variant;

use super::{Answer, bool_operand, text_operand};
use crate::app::agent::{AgentMsg, QueryAnswered};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::NoProjectNamed;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::library::part_search::{PartSearch, ValueFilter, search_parts};
use crate::panels::components_panel::query::{PanelQuery, ValueWord};

/// The most boards a build is priced for, and the most parts a search
/// lists.
const MAX_QUANTITY: u32 = 1_000_000;
const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 500;

/// The whole number operand `name` is, from 1 to `max`; `None` when not
/// given.
fn whole(args: &CommandArgs, name: &str, max: u32) -> Result<Option<u32>, String> {
    let Some(text) = text_operand(args, name) else {
        return Ok(None);
    };
    match text.trim().parse::<u32>() {
        Ok(n) if (1..=max).contains(&n) => Ok(Some(n)),
        _ => Err(format!(
            "`{name}` is a whole number from 1 to {max}, not `{text}`"
        )),
    }
}

/// The text operand `name`, trimmed; `None` when not given or empty.
fn trimmed<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    text_operand(args, name)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

/// The number operand `name` (`1%`, `16V`), read as a query word.
fn number(args: &CommandArgs, name: &str) -> Result<Option<ValueWord>, String> {
    let Some(text) = trimmed(args, name) else {
        return Ok(None);
    };
    ValueWord::parse(text).map(Some).ok_or_else(|| {
        format!("`{name}` is a number with an optional prefix and unit (`1%`, `16V`), not `{text}`")
    })
}

/// A `query_bom` refusal, the reason under the catalog's "not read" line.
fn bom_not_read(why: String) -> signex_i18n::Translatable {
    signex_i18n::Translatable::new(signex_i18n::ids::BOM_QUERY_REFUSED).arg("why", why)
}

impl Signex {
    /// `query_bom document= [quantity=] [currency=] [variant=]`: start the
    /// BOM's build off the UI thread; it is answered when it lands. Refused
    /// at once when the operands name no open project.
    pub(super) fn query_bom(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        match self.start_bom_query(command, args) {
            Ok(started) => started,
            Err(why) => {
                let reason = bom_not_read(why);
                (CommandOutcome::Disabled { reason }, Task::none())
            }
        }
    }

    fn start_bom_query(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> Result<(CommandOutcome, Task<Message>), String> {
        let Some(OperandValue::Document(named)) = args.operand("document") else {
            return Err("`query_bom` needs the operand `document`".to_string());
        };
        let project = self.project_named(named).map_err(|why| match why {
            NoProjectNamed::Unresolved(why) => why,
            NoProjectNamed::NotInProject(name) => {
                format!("`query_bom` prices a project's BOM, and {name} is not part of an open one")
            }
        })?;
        let quantity = whole(args, "quantity", MAX_QUANTITY)?.unwrap_or(1);
        let currency = trimmed(args, "currency").map(str::to_uppercase);
        let data = &project.data;
        let variant = match trimmed(args, "variant") {
            None => data.active_variant.clone().filter(|v| !is_base_variant(v)),
            Some(named) if is_base_variant(named) => None,
            Some(named) => Some(
                data.variant_definitions
                    .iter()
                    .find(|defined| defined.as_str() == named)
                    .cloned()
                    .ok_or_else(|| format!("the project has no variant `{named}`"))?,
            ),
        };
        let job = self.project_bom_job(project, variant.clone());
        let alternates = data.bom_alternates.clone();
        let prices = crate::library::bom_prices::price_book(&self.library);
        let project_path = project.path.clone();
        let header = json!({
            "project": project_path.display().to_string(),
            "variant": variant,
            "source": "cache",
        });
        let waiting = self.wait_for_command(command, args);
        let ticket = waiting.ticket;
        let work = move || -> Result<Value, String> {
            let bom = job()?;
            let order = BuildOrder {
                quantity,
                currency: currency.as_deref(),
            };
            let priced = priced_bom(&bom.table, &prices, &alternates, order);
            let mut answer = serde_json::to_value(&priced)
                .map_err(|error| format!("the BOM could not be written: {error}"))?;
            if let (Value::Object(map), Value::Object(head)) = (&mut answer, header) {
                map.extend(head);
                map.insert("warnings".into(), json!(bom.warnings));
            }
            Ok(answer)
        };
        let task = Task::perform(
            async move {
                tokio::task::spawn_blocking(work)
                    .await
                    .map_err(|error| format!("the BOM did not finish: {error}"))
                    .and_then(|answer| answer)
            },
            move |answer| {
                Message::Agent(AgentMsg::QueryAnswered(Box::new(QueryAnswered {
                    waiting: waiting.clone(),
                    document: project_path.clone(),
                    answer: answer.map_err(bom_not_read),
                })))
            },
        );
        Ok((CommandOutcome::Deferred { ticket }, task))
    }

    /// `search_parts query= [value=] [package=] [tolerance=] [rating=]
    /// [min_stock=] [quantity=] [refresh=] [limit=]`.
    pub(super) fn search_parts_query(&self, args: &CommandArgs) -> Answer {
        if bool_operand(args, "refresh") == Some(true) {
            return Err(
                "`refresh=true` would fetch prices from the distributors, which `search_parts` \
                 does not do; leave `refresh` out to search the cached supplier data"
                    .to_string(),
            );
        }
        // The answer does not follow the window's language: words fold by
        // the invariant (en-US) rules, so `pic` finds `PIC16F` under tr-TR
        // too (#2986).
        let invariant = signex_i18n::I18n::en_us();
        let i18n = invariant.as_ref();
        let value = trimmed(args, "value").map(|text| match ValueWord::parse(text) {
            Some(word) => ValueFilter::Value(word),
            None => ValueFilter::Text(text.to_string()),
        });
        let search = PartSearch {
            query: PanelQuery::parse(i18n, text_operand(args, "query").unwrap_or_default()),
            value,
            package: trimmed(args, "package").map(str::to_string),
            tolerance: number(args, "tolerance")?,
            rating: number(args, "rating")?,
            min_stock: whole(args, "min_stock", u32::MAX)?,
            quantity: whole(args, "quantity", MAX_QUANTITY)?.unwrap_or(1),
            limit: whole(args, "limit", MAX_LIMIT)?.unwrap_or(DEFAULT_LIMIT) as usize,
        };
        Ok(search_parts(&self.library, i18n, &search))
    }
}
