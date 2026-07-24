# Changelog

## [unreleased]

- Replace `crossbeam` with `crossbeam-channel` for newer version.
- Add a grace period to writing to prevent out of order samples.
- Replace `tracing` with `log`.

## v0.1.2

- Add configuration for timeout of write requests.
- Mark registry as sent on client errors.

# v0.1.1

- Skip sending if there are no new samples to write.
- Flatten imports for better diffs.

## v0.1.0

Initial release.
