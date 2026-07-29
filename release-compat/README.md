# Legacy self-update bridge

This directory is included only in release archives.

A3S 0.9.9 through 0.10.10 required every self-update archive to contain a
`support/managed-srt` tree before they would install the new binary. A3S 0.10.11
removed the Anthropic SRT runtime, so omitting that tree made those older
clients unable to update themselves.

The nested package is an inert, fail-closed compatibility marker. Current A3S
versions do not discover or execute it, and it must never acquire sandbox
runtime dependencies or implementation code.
