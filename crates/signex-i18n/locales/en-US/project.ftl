# The Project tab (Project setup section): settings of the project itself.

# Agent journals group (#2253).
project-agent-journals-title = Agent journals
project-agent-journals-keep-in-git = Keep agent journals in git
project-agent-journals-keep-in-git-help = Keeps .signex/agent-*.jsonl in git. Turned off, Signex lists them in its managed .gitignore block when the project is saved; a journal that is already committed stays tracked.
project-agent-journals-summarise = Summarise agent changes in the commit message
project-agent-journals-summarise-help = Saved with the project; it takes effect once commit messages list agent changes, which a later version adds.
project-agent-journals-lines = Journal size (lines)
project-agent-journals-lines-help = Lines kept per journal. Older lines are dropped from the file; they stay in git history only while the journals are kept in git, otherwise they are discarded.
project-agent-journals-lines-invalid = Enter a whole number from { $min } to { $max }; the stored value is unchanged.
