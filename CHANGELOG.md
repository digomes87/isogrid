# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `input::Key::Digit(u8)` for the number row, with `Key::digit` to build one
  safely and `Key::as_digit` to read it back. The macroquad backend reports
  `Key0` through `Key9`. A game cannot bind a toolbar to keys the backend never
  reports, and the number row is what every tile game ends up binding one to.
