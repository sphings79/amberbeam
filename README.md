This branch holds one generated file: `latest.json`, the manifest
AmberBeam reads to find out that a new version exists. It is rewritten
by the release workflow and has no history worth reading.

The manifest is not what makes an update trustworthy. Every download
is checked against the public key built into the program, so pointing
this file somewhere else achieves nothing without the private key.
