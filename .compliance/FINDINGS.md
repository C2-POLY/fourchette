# Findings from the v0 manifest

The v0 manifest was written by hand on Fourchette before any schema existed (C2-POLY/fourchette#238), to test the format planned in C2-POLY/compliance-spec#3. It describes what the code does; it holds no user data. This branch is not meant to be merged.

## What fitted

- The five planned sections (personal data, processors, storage and retention, outbound flows, automated decisions) were enough to describe everything found.
- A `source` on every item worked: each value points to a file, so a reviewer can check it.
- `linked_files` per item is useful: the lag only needs to watch about ten files out of the whole app.

## What did not fit, and what the format needs

1. **Who is the controller.** A local app processes data on the user's machine: the user, not the publisher, decides what goes in. The format assumed a server run by the company. It needs a `controller` field at service level (`organisation`, `user-of-the-app`, `shared`), and `legal_basis` must accept "not applicable" when the organisation is not the controller.
2. **Recipients that are not processors.** GitHub and Anthropic are reached with the user's own accounts (`gh`, `claude`). They are not processors contracted by the organisation, yet the data leaves the machine. The format needs to tell apart `processors` (contracted by the organisation, with a DPA) from `outbound_flows` to third parties chosen by the user, with an `account` field (whose account is used).
3. **Several deliverables in one repo.** The repo holds a Mac app, an iOS app and a website, each with its own flows. A `kind` per service is not enough: either one manifest per deliverable, or a `component` field on each item.
4. **Unknown categories.** Repository content can hold anything. A field category `unknown` (or "whatever the user's data holds") is needed; forcing a category would be false.
5. **Retention without a duration.** Most retention here is an event ("until removed", "until sign-out", "session"), not a duration. Retention needs `duration` (ISO 8601 or `none`/`session`/`unknown`) plus a `trigger` in words.
6. **Country of a recipient.** Transfers outside the EU matter (GitHub, Anthropic: US). `country` belongs on every outbound flow and processor, with the transfer safeguard when known.
7. **AI features.** `automated_decisions` mixed two things: decisions about a person (none here) and AI-generated content (several). The AI Act Art. 50 transparency question is about the second. The format needs a separate `ai_features` list (`generates_content`, `human_review`, `marking`), and keeps `automated_decisions` for decisions about persons.
8. **Credentials.** A token is personal data of a special kind for security. A `credential` category with the storage protection (keychain, device only) is worth having.
9. **Docs that disagree with the code.** The update check runs every hour (`app/src/update.rs`), while `CLAUDE.md` says every six hours. The manifest must follow the code; a declared value that contradicts the code is exactly what provenance should expose.

## Fields to keep in v0.1

`spec_version`, `service` (with `controller` and `components`), `personal_data` (with `fields[].category`, including `unknown` and `credential`), `storage` (with `retention.duration` + `trigger`), `processors`, `outbound_flows` (with `country`, `account`), `automated_decisions`, `ai_features`, `linked_files`, and `source` on every item.

## Dropped or deferred

- `legal_basis` per data item stays, but optional, and only meaningful when the controller is the organisation.
- Security measures per storage: wanted by the GDPR Art. 30 register, but nothing in the code states them; deferred to the processing file of the compliance repo (declared by a person).
