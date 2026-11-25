# Security Policy

## Reporting a vulnerability

Please do not report security problems in public issues, pull requests or
discussions.

Report them privately through GitHub instead:

1. Open the repository's **Security** tab.
2. Choose **Report a vulnerability**.
3. Describe the problem, the version or commit you saw it on, and the steps
   that reproduce it. A sample file that triggers it helps, if you can share
   one.

Only the maintainers can see the report. We will acknowledge it, keep you
updated while we work on a fix, and credit you in the advisory unless you ask
us not to.

## What counts

Anything that lets a file, a library, a project or a network response make
Signex do something the user did not ask for. For example:

- running code or writing files outside the project when a design, library or
  project file is opened;
- reading or sending data the user did not choose to share;
- a crash or hang from a crafted file that can be triggered without the user
  doing anything unusual;
- weaknesses in the release packages or the update path.

Ordinary bugs, including crashes on malformed input that need deliberate
effort to trigger, can go in a normal issue.

## Supported versions

Fixes go into the next release. Use the latest release to get them; older
releases are not patched.
