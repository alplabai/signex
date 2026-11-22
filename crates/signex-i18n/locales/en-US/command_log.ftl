# The Command Log panel (#2458).
panels-kind-command-log = Command Log
command-log-pause = Pause logging
command-log-resume = Resume logging
command-log-save = Save log as…
command-log-clear = Clear
command-log-more = More
command-log-show-view-commands = Show view commands
command-log-filter-placeholder = Filter by command, id or argument
command-log-source = Source
command-log-source-gui = GUI
command-log-source-cli = CLI
command-log-source-agent = Agent socket
command-log-pill-gui = GUI
command-log-pill-cli = CLI
command-log-pill-agent = Agent
command-log-result-all = All
command-log-result-failed = Failed { $count }
command-log-result-undone = Undone { $count }
command-log-follow-newest = Follow newest
command-log-column-time = Time
command-log-column-source = Source
command-log-column-command = Command
command-log-column-arguments = Arguments
command-log-column-result = Result
command-log-column-took = Took
command-log-no-value = —
command-log-done = Done
command-log-took-ms = { $ms } ms
command-log-took-s = { $s } s
command-log-empty-title = No commands yet in this session
command-log-empty-body = Every command you run from a menu, key, tool, the CLI or the agent socket appears here with its arguments and result.
command-log-empty-key = signex-cli commands
command-log-empty-key-caption = lists the catalog
command-log-no-match = No commands match these filters.
command-log-detail-result = Result
command-log-copy-error = Copy error
command-log-copied = Copied
command-log-detail-time = Time
command-log-detail-client = Client
command-log-detail-took = Took
command-log-detail-undo = Undo
command-log-undo-nothing-changed = Nothing changed
command-log-undo-undone-at = Undone at { $time }
command-log-undo-read-only = Not undoable (read only)
command-log-undo-undoable = Undoable
command-log-detail-arguments = Arguments
command-log-no-arguments = No arguments
command-log-copy-as-cli = Copy as CLI
command-log-copy = Copy
command-log-replay = Replay
command-log-jump-to = Jump to { $target }
command-log-jump-to-object = Jump to object
command-log-replay-failed-hint = Replay runs it again with the same arguments; it fails the same way until the cause is fixed.
command-log-replay-no-targets = Not replayed: this row recorded no targets, and Replay never acts on the current selection.
# $command is the command's label.
command-log-replay-not-finished = Not replayed: { $command } was started but never finished, so it recorded nothing to repeat.
command-log-replay-nothing-to-repeat = Not replayed: { $command } has nothing to repeat: it is a step of a tool in progress, a pick at the pointer, or a named save.
command-log-replay-made-objects = Not replayed: { $command } made new objects from values only its tool held.
command-log-replay-tool-active = Not replayed: a tool is in progress, and running this on { $document } would disturb it. Finish or cancel the tool first.
command-log-replay-no-change = The replayed command changed nothing.
# $missing lists each missing item's kind and id.
command-log-replay-target-gone = Not replayed: { $count } recorded targets are no longer in { $document }: { $missing }
command-log-replay-document-closed = Not replayed: { $document } is not open.
command-log-replay-document-busy = Not replayed: { $document } cannot be brought forward right now.
command-log-client-window = Signex window 1
command-log-client-undocked = Signex window of { $document }
command-log-client-pid = pid { $pid }
command-log-footer-listening = Logging · agent socket listening on { $endpoint }
command-log-footer-off = Logging · agent socket off
command-log-footer-paused = Paused · new commands are not logged
command-log-footer-count = { $shown } of { $total } commands
command-log-footer-none = 0 commands
command-log-footer-session = Kept for this session only
command-log-cli-no-project = <project.snxprj>
command-log-save-title = Save Command Log
command-log-save-filter = Command Log (JSON Lines)

# Messages lines.
# { $command } is the id a row was logged under.
command-log-not-a-command = Command Log: `{ $command }` is not a command id
# { $name } is the designator or net a row names.
command-log-not-in-view = Command Log: { $name } is not on the sheet in view
command-log-saved-to = Command Log saved to { $path }
# { $why } is the system's reason.
command-log-not-saved = Command Log not saved: { $why }
