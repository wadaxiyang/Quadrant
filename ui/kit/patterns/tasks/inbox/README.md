# Inbox pattern package

`InboxItem` is a presentation-only struct and `InboxPane` communicates only
through properties and callbacks. The directory does not import Product views,
Rust domain types, repositories, application commands, Gallery helpers, or live
storage.

Copy this directory together with these Kit dependencies when embedding it in a
different Slint root:

- `foundation/theme.slint`, `foundation/constants.slint`, and
  `foundation/fluent_icons.slint`
- `primitives/badge.slint`, `fluent_button.slint`, `icon_button.slint`,
  `segment_button.slint`, and `surface_card.slint`
- `patterns/page/section_header.slint`
- `patterns/tasks/task_row_shell.slint`

Product code should import `InboxItem` and `InboxPane` from `ui/kit/kit.slint`
and adapt its application projection into `InboxItem` outside the component.
