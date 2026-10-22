# Security policy

## Supported versions

MK Perth is in early development (0.x) and has no release yet. Security fixes go into `main`;
once releases exist, only the latest is supported.

## Reporting a vulnerability

Report it **privately**: on GitHub, open the repository's **Security** tab and choose **Report a
vulnerability** (a GitHub Security Advisory). Please don't open a public issue.

Include what you can: the commit or version, the platform, the input or the steps that trigger
it, and what an attacker gains. You'll get an answer within a week. Once a fix lands, the
advisory is published with credit to you, unless you'd rather not be named.

## What counts

MK Perth opens project files people receive from others, so a file must never be able to do
harm:

- a Perth JSON project (and, as they come, P6 XML, Microsoft Project XML or XER files) that
  crashes the program, hangs it, exhausts memory, or writes outside the paths the person chose;
- a network or a calendar built to make the scheduler loop or overflow (loops of relationships or
  of parents, calendars with no working day, dates at the ends of time);
- the MCP server, when it comes, doing more than the commands it exposes.

A crash on a hostile input is a bug we treat as a security issue (the project's "never crash"
rule): report it privately if you think it can be exploited, as a normal issue otherwise.
