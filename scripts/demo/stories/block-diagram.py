"""Story: Block Diagram. Pending: its commands land with the Block Diagram
batch. Until then `record_all.sh` skips it with the reason below; when the
batch merges, write `prepare` and `run` from its catalog commands and drop
`PENDING`."""

TITLE = "Block Diagram"
PENDING = "the Block Diagram batch has not merged yet; its commands are not in the catalog"


def prepare(ctx):
    raise NotImplementedError(PENDING)


def run(ctx):
    raise NotImplementedError(PENDING)
