# DeviceDeck development notes

This repository is the DeviceDeck fork of QuickBuds. `main` is the release branch, `develop` is the integration branch, and feature changes merge to `develop` through PRs. The umbrella repo pins this fork as a submodule.

Keep QuickBuds' GPL-3.0 license and notices. Preserve Android/Windows/Linux support and upstream capabilities while adding separate device protocol modules. DeviceDeck model support must have per-model evidence; do not claim hardware validation based only on model catalog entries.

Never reuse upstream package signing credentials or update infrastructure. Before shipping a fork build, give it a distinct application identity and ensure reports, analytics, update endpoints, and privacy disclosures match DeviceDeck's own configuration.
