use iced::Task;

use super::*;

impl Signex {
    pub(crate) fn open_transmission_line_calculator(&mut self) -> Task<Message> {
        if self
            .ui_state
            .windows
            .values()
            .any(|kind| matches!(kind, super::state::WindowKind::TransmissionLineCalculator))
        {
            return Task::none();
        }

        let (id, open_task) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(640.0, 760.0),
            resizable: true,
            decorations: true,
            ..Default::default()
        });
        self.ui_state
            .windows
            .insert(id, super::state::WindowKind::TransmissionLineCalculator);
        open_task.map(|id| Message::Window(WindowMsg::TransmissionLineCalculatorOpened(id)))
    }

    pub(crate) fn dispatch_transmission_line_calculator_message(
        &mut self,
        message: crate::transmission_line_calculator::SmithChartMessage,
    ) -> Task<Message> {
        use crate::transmission_line_calculator::SmithChartMessage;

        match message {
            SmithChartMessage::ImportSParameterFile => Task::perform(
                async {
                    let Some(file) = rfd::AsyncFileDialog::new()
                        .add_filter("Touchstone", &["s1p", "s2p", "snp", "txt"])
                        .pick_file()
                        .await
                    else {
                        return Ok(None);
                    };
                    let bytes = file.read().await;
                    String::from_utf8(bytes)
                        .map(Some)
                        .map_err(|err| format!("Touchstone file is not UTF-8: {err}"))
                },
                |result| {
                    Message::TransmissionLineCalculator(SmithChartMessage::SParameterFileLoaded(
                        result,
                    ))
                },
            ),
            SmithChartMessage::SaveCsvFile => {
                match self
                    .ui_state
                    .transmission_line_calculator
                    .generated_csv_export()
                {
                    Ok((file_name, csv)) => Task::perform(
                        async move {
                            let Some(file) = rfd::AsyncFileDialog::new()
                                .add_filter("CSV", &["csv"])
                                .set_file_name(file_name)
                                .save_file()
                                .await
                            else {
                                return Ok(None);
                            };
                            let path = file.path().to_path_buf();
                            std::fs::write(&path, csv.as_bytes())
                                .map(|()| Some(path.display().to_string()))
                                .map_err(|err| format!("Failed to save CSV: {err}"))
                        },
                        |result| {
                            Message::TransmissionLineCalculator(SmithChartMessage::CsvFileSaved(
                                result,
                            ))
                        },
                    ),
                    Err(err) => {
                        self.ui_state
                            .transmission_line_calculator
                            .update(SmithChartMessage::CsvFileSaved(Err(err)));
                        Task::none()
                    }
                }
            }
            SmithChartMessage::SaveSvgFile => {
                match self
                    .ui_state
                    .transmission_line_calculator
                    .generated_svg_export()
                {
                    Ok(svg) => Task::perform(
                        async move {
                            let Some(file) = rfd::AsyncFileDialog::new()
                                .add_filter("SVG", &["svg"])
                                .set_file_name("smith_chart.svg")
                                .save_file()
                                .await
                            else {
                                return Ok(None);
                            };
                            let path = file.path().to_path_buf();
                            std::fs::write(&path, svg.as_bytes())
                                .map(|()| Some(path.display().to_string()))
                                .map_err(|err| format!("Failed to save SVG: {err}"))
                        },
                        |result| {
                            Message::TransmissionLineCalculator(SmithChartMessage::SvgFileSaved(
                                result,
                            ))
                        },
                    ),
                    Err(err) => {
                        self.ui_state
                            .transmission_line_calculator
                            .update(SmithChartMessage::SvgFileSaved(Err(err)));
                        Task::none()
                    }
                }
            }
            message => {
                self.ui_state.transmission_line_calculator.update(message);
                Task::none()
            }
        }
    }
}
