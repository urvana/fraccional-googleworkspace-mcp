# OAuth scope reduction for one Google sign-in

Research note, 2026-11-21. No code was changed. Every claim has a source tag. Tags such as [D-chat] (Discovery doc) and [P-chat-authz] (Google page) are defined in the Sources section at the end. A line marked "inference" is my reading, not a Google statement.

## 1. Summary

Goal: one `gws auth login --scopes ...` must request at most 47 scopes (`MAX_LOGIN_SCOPES`, `scopes.rs:32` [R-scopes]).

What I found:

- The `X.readonly` to `X` rule is only a small part of the overlap. The biggest extra case is `drive`: it is accepted by every method of the Docs, Sheets, Slides and Forms scopes (6 scopes saved when Drive Write is selected) [D-docs], [D-sheets], [D-slides], [D-forms], [P-docs-auth], [P-sheets-scopes], [P-slides-scopes].
- Lossless rules alone are not enough for the whole catalogue. The whole catalogue goes from 69 to 56 scopes in one request. That is still over 47.
- Two scopes are not useful for a normal user sign-in. `gmail.settings.sharing` is only usable by a service account with domain-wide delegation [D-gmail], [P-gmail-scopes]. The `chat.admin.*` scopes are a separate administrator mode [P-chat-authz].
- Two sign-ins fit with no feature loss: a main sign-in (Workspace plus business) and an admin sign-in (Admin SDK, Reports, Cloud Identity, `chat.admin.*`). One sign-in for everything fits only if rare scopes are dropped.
- Method coverage is not proof of safe removal. Four cases look "covered" in Discovery but are not, or may not be (Meet `created`, `chat.admin.*`, `chat.delete`, and People `user.*.read`, which is unverified). I list them in section 2.0.

Baseline from the working-tree `scopes.rs`: the whole catalogue is **69** scopes, not "about 67": 40 Workspace (11 services, including Tasks) + 22 Admin SDK, Reports and Cloud Identity + 7 business. The 10-service Full selection is **39**, as in the brief [R-scopes].

### 1.1 Recommendations, ranked by scopes saved in the whole catalogue

Savings are measured alone, from today's baseline. They do not add up exactly because some rules overlap.

| # | recommendation | type | saved: 10-service Full | saved: whole catalogue | what a person loses |
|---|---|---|---|---|---|
| 1 | Move Admin SDK (Directory, Groups), Reports and Cloud Identity to a separate admin sign-in | move: admin | 0 | 22 leave the main request (19 after rule 6) | Nothing, if admin features use their own sign-in. Needs a new UI flow |
| 2 | Let a requested `drive` cover `documents`, `spreadsheets`, `presentations`, `forms.body`, `forms.responses.readonly` and `drive.scripts` (and `drive.readonly` cover the four `.readonly` ones) | drop: covered | 6 | 6 | Nothing at method level. See 2.0 for how well each pair is confirmed |
| 3 | Drop the six People `user.*.read` scopes | drop: rare | 6 | 6 | Possibly profile fields of `people/me` (birthday, gender, addresses, organization, phone numbers, emails). Unverified (inference) |
| 4 | Drop five rare Chat scopes: `chat.users.spacesettings`, `chat.users.availability`, `chat.users.sections`, `chat.customemojis`, `chat.delete` | drop: rare | 5 | 5 | Space notification settings, availability status, sidebar sections, custom emoji, deleting a whole space |
| 5 | Move `chat.admin.spaces`, `chat.admin.memberships`, `chat.admin.delete` to the admin sign-in | move: admin | 3 | 3 | Chat admin mode (`useAdminAccess`) in the main sign-in |
| 6 | Use the Admin SDK coverage pairs: `user` covers `user.alias`, `device.mobile` covers `device.mobile.action`, `group` covers `group.member` | drop: covered | 0 | 3 | Nothing at method level (Discovery only) |
| 7 | Small lossless rules: `mail.google.com/` covers `gmail.readonly`; `chat.messages` covers `chat.messages.reactions`; `chat.spaces` covers `chat.spaces.pins` | drop: covered | 3 | 3 | Nothing |
| 8 | Drop `gmail.settings.sharing` (service account only) | drop: admin-only | 1 | 1 | Nothing for a user sign-in |
| 9 | Drop `drive.apps.readonly` and `cloud-identity.devices.lookup` | drop: rare | 1 | 2 | Listing installed Drive apps; looking up the caller's own devices |

### 1.2 New counts

One sign-in. Counts are scopes in the request, after each step is added.

| step | 10-service Full | whole catalogue, one sign-in |
|---|---|---|
| Today (current code) | 39 | 69 |
| + A: lossless coverage rules | 30 | 57 |
| + A + B1: drop `gmail.settings.sharing` | 29 | 56 |
| + A + B1 + B2: `chat.admin.*` moved to the admin sign-in (not counted here) | 26 | 53 |
| + A + B1 + B2 + C: rare drops | 14 | 40 |
| Alternative: A + B1 + C, with `chat.admin.*` kept in the request | 17 | 43 |

Two sign-ins. "Main" is Workspace plus business. "Admin" is Admin SDK (Directory, Groups), Reports and Cloud Identity. From the `A + B1 + B2` row, `chat.admin.*` sits in the admin sign-in.

| step | main sign-in | admin sign-in |
|---|---|---|
| Admin families split only (today) | 47 | 22 |
| + A | 38 | 19 |
| + A + B1 | 37 | 19 |
| + A + B1 + B2 (chat.admin.* to admin) | 34 | 22 |
| + A + B1 + B2 + C | 22 | 21 |

Step names: A = lossless coverage rules (rules 2, 6, 7 above). B1 = drop `gmail.settings.sharing` (rule 8). B2 = move `chat.admin.*` out of the main request (rule 5). C = rare drops (rules 3, 4, 9).

Recommended path: **A + B1 + B2 with two sign-ins.** It loses no feature that a user sign-in can use, and both requests fit: the 10-service Full selection has 26 scopes; the whole catalogue has 34 (main) + 22 (admin). Add C only if you want a lighter consent screen or a single sign-in (43 scopes for the whole catalogue with `chat.admin.*` kept, 40 with it moved).

The two-sign-in option depends on two facts that I could not verify. See section 4, questions 1 and 2.

### 1.3 Implementation notes

These are not code changes. They show what each recommendation needs.

- `has_scope` (`scopes.rs:418-424`) knows only `X.readonly` to `X`. If a login leaves out `documents` because `drive` is present, `selection_from_scopes` (`scopes.rs:391-407`) reports Docs Write as not granted. Each new coverage rule must also go into `has_scope` [R-scopes].
- Remove `gmail.settings.sharing` from `level_scopes` (`scopes.rs:159-280`) and from the test tables. Do not filter it at login [R-scopes].
- Moving `chat.admin.*` and the Admin SDK families needs a separate login and a UI choice. Today the UI already marks the Admin SDK, Reports and Cloud Identity rows as "need an administrator" (`availability_reason`) [R-scopes].
- Read-only selections also gain: with Drive Read, `drive.readonly` covers `documents.readonly`, `spreadsheets.readonly`, `presentations.readonly` and `forms.body.readonly` (4 scopes). It does not cover `forms.responses.readonly` (4 of 6 methods; it misses `responses.get` and `responses.list`) [D-forms].

## 2. Tables

Column notes:

- "covered by" lists catalogue scopes A where every method that accepts this scope also accepts A ("full"), and scopes that accept at least half of its methods ("partial"). Numbers are methods covered / methods that accept this scope. This is method coverage only.
- "admin-only?" says whether Google states that a normal user cannot use the scope.
- "class (info)" is the sensitivity class that a fetched Google page states. It is informative only.
- A recommendation of "keep (already dropped when X is requested)" means the current `X.readonly` rule handles it.
- Labels: keep, drop: covered, drop: rare, move: admin. I added "drop: admin-only" for a scope that only a service account with domain-wide delegation can use (`gmail.settings.sharing`). In the admin-only column, "mostly" means admin-only except a documented public view.

### 2.0 Evidence for each lossless rule, and four traps

Rule evidence. "Prose" means a Google page that I fetched states the same thing as Discovery.

| rule (scope B is covered by A) | method coverage | prose confirmation | status |
|---|---|---|---|
| `gmail.readonly` by `https://mail.google.com/` | 32/32 [D-gmail] | [P-gmail-scopes] describes `mail.google.com/` as "Read, compose, send, and permanently delete all your email" and says all other actions can use less permissive scopes | Discovery plus a consistent description |
| `documents` by `drive` | 3/3 [D-docs] | [P-docs-auth] lists `drive` for the Docs API | confirmed |
| `spreadsheets` by `drive` | 17/17 [D-sheets] | [P-sheets-scopes] lists `drive` | confirmed |
| `presentations` by `drive` | 5/5 [D-slides] | [P-slides-scopes] lists `drive` | confirmed |
| `documents.readonly`, `spreadsheets.readonly`, `presentations.readonly` by `drive.readonly` | 1/1, 3/3, 3/3 | the same three pages list `drive.readonly` | confirmed |
| `forms.body`, `forms.responses.readonly` by `drive`; `forms.body.readonly` by `drive.readonly` | 8/8, 6/6, 5/5 [D-forms] | no Forms scope page found | inference (Discovery only) |
| `drive.scripts` by `drive` | 1/1 (`files.update`) [D-drive] | none | inference (Discovery only) |
| `chat.messages.reactions` by `chat.messages`; `.reactions.readonly` by `chat.messages.readonly` | 5/5, 3/3 [D-chat] | the Chat method table lists `chat.messages` for create, list and delete reaction [P-chat-authz] | confirmed |
| `chat.spaces.pins` by `chat.spaces`; `.pins.readonly` by `chat.spaces.readonly` | 3/3, 1/1 [D-chat] | the Chat method table lists `chat.spaces` for pin, unpin and list pins [P-chat-authz] | confirmed |
| `admin.directory.user.alias` by `admin.directory.user`; `device.mobile.action` by `device.mobile`; `group.member` by `group` (and the `.readonly` pairs) | 5/5, 3/3, 7/7 [D-directory] | none | inference (Discovery only) |

Pairs that look covered in Discovery but must **not** be treated as covered:

| pair | why it is a trap |
|---|---|
| `meetings.space.readonly` and `meetings.space.settings` by `meetings.space.created` | `created` accepts all 17 and both methods. But the Meet guide says `created` applies to "meeting spaces created by your app", and `readonly` to "any meeting space the user has access to" [P-meet-scopes]. Keep all three: method coverage only |
| `chat.admin.spaces`, `chat.admin.memberships`, `chat.admin.delete` and their `.readonly` variants, by `chat.spaces`, `chat.memberships`, `chat.delete` | Discovery lists the user scopes on the same methods (3/3 and 5/5). The Chat method table puts "User authentication using administrator privileges" on `chat.admin.*` only [D-chat], [P-chat-authz]. Treat as admin mode, not as covered |
| `chat.delete` and `chat.admin.delete` | They cover each other at method level (both on `spaces.delete`, 1/1). The Chat table separates user mode and admin mode [P-chat-authz] |
| the six `user.*.read` scopes | Each covers the other eight People scopes (2/2) because only `people.get` and `people.getBatchGet` accept them [D-people]. The Person reference has two source families (profile and contact) and no per-field scope notes [P-people-person], [P-people-source]. I infer that `user.*` gate profile-source fields. Not verified |

Other near misses: `https://mail.google.com/` covers only 19 of 37 `gmail.settings.basic` methods (the reads) [D-gmail]. `analytics.edit` covers 23/23 Analytics Admin API methods but 0/11 Analytics Data API methods [D-analyticsadmin], [D-analyticsdata]. `contacts` covers only 3 of 5 `contacts.other.readonly` methods [D-people].

### 2.1 Gmail

Discovery: [D-gmail] r20261116. Pages: [P-gmail-scopes].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `gmail.readonly` | Read | full: `https://mail.google.com/` 32/32; partial: `gmail.settings.basic` 19/32 (of 32 methods) | no | drop: covered by `https://mail.google.com/` (32/32) when Gmail Write is requested; otherwise keep | restricted | [D-gmail], [P-gmail-scopes] |
| `https://mail.google.com/` | Write | partial: `gmail.readonly` 32/53 (of 53 methods) | no | keep: it covers `gmail.readonly` fully but only 19 of 37 `gmail.settings.basic` methods (the read ones) | restricted | [D-gmail], [P-gmail-scopes] |
| `gmail.settings.basic` | Write | partial: `https://mail.google.com/` 19/37, `gmail.readonly` 19/37 (of 37 methods) | no | keep: needed for settings writes (filters, vacation, IMAP, POP, language, S/MIME, CSE); `mail.google.com` does not cover them | restricted | [D-gmail] |
| `gmail.settings.sharing` | Write | partial: `gmail.settings.basic` 18/26 (of 26 methods) | yes: service account with domain-wide delegation only | drop: admin-only. The 8 methods that need only this scope say "only available to service account clients that have been delegated domain-wide authority". A user sign-in cannot call them | restricted | [D-gmail], [P-gmail-scopes] |

### 2.2 Calendar

Discovery: [D-calendar] r20261118. Pages: [P-cal-auth].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `calendar.readonly` | Read | full: `calendar` 15/15 (of 15 methods) | no | keep (already dropped when `calendar` is requested; 15/15) | not stated on fetched page | [D-calendar] |
| `calendar` | Write | none (of 38 methods) | no | keep | not stated on fetched page | [D-calendar] |

### 2.3 Chat

Discovery: [D-chat] r20261120. Pages: [P-chat-authz].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `chat.spaces.readonly` | Read | full: `chat.spaces` 7/7 (of 7 methods) | no | keep (already dropped when `chat.spaces` is requested) | sensitive | [D-chat], [P-chat-authz] |
| `chat.messages.readonly` | Read | full: `chat.messages` 7/7 (of 7 methods) | no | keep (already dropped when `chat.messages` is requested) | restricted | [D-chat], [P-chat-authz] |
| `chat.memberships.readonly` | Read | full: `chat.memberships` 5/5 (of 5 methods) | no | keep (already dropped when `chat.memberships` is requested) | sensitive | [D-chat], [P-chat-authz] |
| `chat.messages.reactions.readonly` | Read | full: `chat.messages` 3/3, `chat.messages.reactions` 3/3, `chat.messages.readonly` 3/3; partial: `chat.memberships` 2/3, `chat.memberships.readonly` 2/3, `chat.spaces` 2/3, `chat.spaces.readonly` 2/3 (of 3 methods) | no | drop: covered by `chat.messages.readonly` (3/3) or `chat.messages`; Chat table lists both for "List reactions" | sensitive | [D-chat], [P-chat-authz] |
| `chat.users.readstate.readonly` | Read | full: `chat.users.readstate` 2/2 (of 2 methods) | no | keep (already dropped when `chat.users.readstate` is requested) | sensitive | [D-chat], [P-chat-authz] |
| `chat.users.sections.readonly` | Read | full: `chat.users.sections` 2/2 (of 2 methods) | no | drop: rare. 2 methods: list sections and section items | sensitive | [D-chat], [P-chat-authz] |
| `chat.users.availability.readonly` | Read | full: `chat.users.availability` 1/1 (of 1 method) | no | drop: rare. 1 method: get availability. Not in the Chat scope tables | not stated on fetched page | [D-chat] |
| `chat.spaces.pins.readonly` | Read | full: `chat.spaces` 1/1, `chat.spaces.pins` 1/1, `chat.spaces.readonly` 1/1 (of 1 method) | no | drop: covered by `chat.spaces.readonly` (1/1) or `chat.spaces`; Chat table lists both for "List pinned messages" | not stated on fetched page | [D-chat], [P-chat-authz] |
| `chat.customemojis.readonly` | Read | full: `chat.customemojis` 2/2 (of 2 methods) | no | drop: rare. 2 methods: get and list custom emoji | sensitive | [D-chat], [P-chat-authz] |
| `chat.admin.spaces.readonly` | Read | full: `chat.admin.spaces` 2/2, `chat.spaces` 2/2, `chat.spaces.readonly` 2/2 (of 2 methods) - method-level only; Chat doc puts these in "administrator privileges" mode | yes: needs `useAdminAccess` (administrator privileges) | move: admin. Not "covered": Discovery lists `chat.spaces*` on the same methods, but the Chat table puts admin mode on `chat.admin.*` only | sensitive | [D-chat], [P-chat-authz] |
| `chat.admin.memberships.readonly` | Read | full: `chat.admin.memberships` 2/2, `chat.memberships` 2/2, `chat.memberships.readonly` 2/2 (of 2 methods) - method-level only; Chat doc puts these in "administrator privileges" mode | yes: needs `useAdminAccess` | move: admin (same reason) | sensitive | [D-chat], [P-chat-authz] |
| `chat.spaces` | Write | partial: `chat.spaces.readonly` 7/12 (of 12 methods) | no | keep: also covers `chat.spaces.pins` (3/3) and `chat.spaces.readonly` | sensitive | [D-chat], [P-chat-authz] |
| `chat.memberships` | Write | partial: `chat.admin.memberships` 5/8, `chat.memberships.readonly` 5/8 (of 8 methods) | no | keep | sensitive | [D-chat], [P-chat-authz] |
| `chat.messages` | Write | partial: `chat.messages.readonly` 7/14 (of 14 methods) | no | keep: also covers `chat.messages.reactions` (5/5) and `chat.messages.readonly` | restricted | [D-chat], [P-chat-authz] |
| `chat.messages.reactions` | Write | full: `chat.messages` 5/5; partial: `chat.messages.reactions.readonly` 3/5, `chat.messages.readonly` 3/5 (of 5 methods) | no | drop: covered by `chat.messages` (5/5); Chat table lists `chat.messages` for create, list and delete reaction | sensitive | [D-chat], [P-chat-authz] |
| `chat.users.readstate` | Write | partial: `chat.users.readstate.readonly` 2/4 (of 4 methods) | no | keep (4 methods: get and update space and thread read state; no other scope covers them) | sensitive | [D-chat], [P-chat-authz] |
| `chat.users.sections` | Write | none (of 7 methods) | no | drop: rare. 7 methods: create, list, update, delete, position sections; list and move section items | sensitive | [D-chat], [P-chat-authz] |
| `chat.users.availability` | Write | none (of 5 methods) | no | drop: rare. 5 methods: get, patch, markAsActive, markAsAway, markAsDoNotDisturb. Not in the Chat scope tables | not stated on fetched page | [D-chat] |
| `chat.users.spacesettings` | Write | none (of 2 methods) | no | drop: rare. 2 methods: get and patch space notification setting | sensitive | [D-chat], [P-chat-authz] |
| `chat.spaces.pins` | Write | full: `chat.spaces` 3/3 (of 3 methods) | no | drop: covered by `chat.spaces` (3/3); Chat table lists `chat.spaces` for pin, unpin and list pins | not stated on fetched page | [D-chat], [P-chat-authz] |
| `chat.customemojis` | Write | partial: `chat.customemojis.readonly` 2/4 (of 4 methods) | no | drop: rare. 4 methods: get, list, create, delete custom emoji | sensitive | [D-chat], [P-chat-authz] |
| `chat.delete` | Write | full: `chat.admin.delete` 1/1 (same method `spaces.delete`; method-level only) | no | drop: rare. 1 method: `spaces.delete` (delete a whole space, user mode). Restricted scope. Not covered by `chat.admin.delete` (admin mode) | restricted | [D-chat], [P-chat-authz] |
| `chat.admin.spaces` | Write | full: `chat.spaces` 3/3; partial: `chat.admin.spaces.readonly` 2/3, `chat.spaces.readonly` 2/3 (of 3 methods) - method-level only; Chat doc puts these in "administrator privileges" mode | yes: needs `useAdminAccess` | move: admin. Method-level full cover by `chat.spaces` (3/3) is not valid because of admin mode | sensitive | [D-chat], [P-chat-authz] |
| `chat.admin.memberships` | Write | full: `chat.memberships` 5/5 (of 5 methods) - method-level only; Chat doc puts these in "administrator privileges" mode | yes: needs `useAdminAccess` | move: admin (same reason) | sensitive | [D-chat], [P-chat-authz] |
| `chat.admin.delete` | Write | full: `chat.delete` 1/1 (method-level only; Chat doc separates user and admin mode) | yes: needs `useAdminAccess` | move: admin | restricted | [D-chat], [P-chat-authz] |

### 2.4 Meet

Discovery: [D-meet] r20261117. Pages: [P-meet-scopes].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `meetings.space.readonly` | Read | full: `meetings.space.created` 17/17 (of 17 methods) | no | keep: method coverage only. `meetings.space.created` accepts all 17 methods, but the Meet guide limits it to "meeting spaces created by your app"; `readonly` reads "any meeting space the user has access to" | sensitive | [D-meet], [P-meet-scopes] |
| `meetings.space.created` | Write | partial: `meetings.space.readonly` 17/24 (of 24 methods) | no | keep | sensitive | [D-meet], [P-meet-scopes] |
| `meetings.space.settings` | Write | full: `meetings.space.created` 2/2; partial: `meetings.space.readonly` 1/2 (of 2 methods) | no | keep: method coverage only. `created` accepts both methods (`spaces.get`, `spaces.patch`), but `settings` is for "all of your Google Meet calls". Non-sensitive | non-sensitive | [D-meet], [P-meet-scopes] |

### 2.5 Drive

Discovery: [D-drive] v3 r20261117. Pages: [P-drive-scopes].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `drive.readonly` | Read | full: `drive` 29/29 (of 29 methods) | no | keep (already dropped when `drive` is requested; 29/29) | restricted | [D-drive], [P-drive-scopes] |
| `drive.apps.readonly` | Read | partial: `drive` 1/2, `drive.readonly` 1/2 (of 2 methods) | no | drop: rare. Only `apps.list` needs it (`apps.get` also accepts `drive` and `drive.readonly`). Lists the apps installed in Drive | sensitive | [D-drive], [P-drive-scopes] |
| `drive` | Write | none (of 63 methods) | no | keep: also covers Docs, Sheets, Slides and Forms scopes (see those tables) | restricted | [D-drive], [P-drive-scopes] |
| `drive.scripts` | Write | full: `drive` 1/1 (of 1 method) | no | drop: covered by `drive` (1/1). Only `files.update` uses it. Discovery only; no fetched prose confirms it | restricted | [D-drive], [P-drive-scopes] |

### 2.6 Docs

Discovery: [D-docs] r20261118. Pages: [P-docs-auth].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `documents.readonly` | Read | full: `documents` 1/1, `drive` 1/1, `drive.readonly` 1/1 (of 1 method) | no | drop: covered by `drive.readonly` (1/1) or `drive`, `documents`; Docs scope table lists both Drive scopes | sensitive | [D-docs], [P-docs-auth] |
| `documents` | Write | full: `drive` 3/3 (of 3 methods) | no | drop: covered by `drive` (3/3) when Drive Write is requested; Docs scope table lists `drive` | sensitive | [D-docs], [P-docs-auth] |

### 2.7 Sheets

Discovery: [D-sheets] r20261118. Pages: [P-sheets-scopes].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `spreadsheets.readonly` | Read | full: `drive` 3/3, `drive.readonly` 3/3, `spreadsheets` 3/3 (of 3 methods) | no | drop: covered by `drive.readonly` (3/3) or `drive`, `spreadsheets` | sensitive | [D-sheets], [P-sheets-scopes] |
| `spreadsheets` | Write | full: `drive` 17/17 (of 17 methods) | no | drop: covered by `drive` (17/17) when Drive Write is requested; Sheets scope table lists `drive` | sensitive | [D-sheets], [P-sheets-scopes] |

### 2.8 Slides

Discovery: [D-slides] r20261118. Pages: [P-slides-scopes].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `presentations.readonly` | Read | full: `drive` 3/3, `drive.readonly` 3/3, `presentations` 3/3 (of 3 methods) | no | drop: covered by `drive.readonly` (3/3) or `drive`, `presentations` | sensitive | [D-slides], [P-slides-scopes] |
| `presentations` | Write | full: `drive` 5/5; partial: `drive.readonly` 4/5, `presentations.readonly` 3/5 (of 5 methods) | no | drop: covered by `drive` (5/5) when Drive Write is requested; Slides scope table lists `drive` | sensitive | [D-slides], [P-slides-scopes] |

### 2.9 Forms

Discovery: [D-forms] r20261115. Pages: none fetched (no Forms scope page found).

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `forms.body.readonly` | Read | full: `drive` 5/5, `drive.readonly` 5/5, `forms.body` 5/5; partial: `forms.responses.readonly` 4/5 (of 5 methods) | no | drop: covered by `drive.readonly` (5/5) or `drive`, `forms.body`. Discovery only | not stated on fetched page | [D-forms] |
| `forms.responses.readonly` | Read | full: `drive` 6/6; partial: `drive.readonly` 4/6, `forms.body` 4/6, `forms.body.readonly` 4/6 (of 6 methods) | no | drop: covered by `drive` (6/6) when Drive Write is requested. `drive.readonly` covers only 4/6 (the 4 `watches` methods, not `responses.get` and `responses.list`), so keep this scope for Drive Read. Discovery only | not stated on fetched page | [D-forms] |
| `forms.body` | Write | full: `drive` 8/8; partial: `drive.readonly` 5/8, `forms.body.readonly` 5/8, `forms.responses.readonly` 4/8 (of 8 methods) | no | drop: covered by `drive` (8/8) when Drive Write is requested. Discovery only | not stated on fetched page | [D-forms] |

### 2.10 Tasks

Discovery: [D-tasks] r20261117. Pages: none fetched.

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `tasks.readonly` | Read | full: `tasks` 4/4 (of 4 methods) | no | keep (already dropped when `tasks` is requested; 4/4) | not stated on fetched page | [D-tasks] |
| `tasks` | Write | none (of 14 methods) | no | keep | not stated on fetched page | [D-tasks] |

### 2.11 People

Discovery: [D-people] r20261118. Pages: [P-people-get], [P-people-person], [P-people-source].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `contacts.readonly` | Read | full: `contacts` 7/7 (of 7 methods) | no | keep (already dropped when `contacts` is requested; 7/7) | not stated on fetched page | [D-people], [P-people-get] |
| `contacts.other.readonly` | Read | partial: `contacts` 3/5 (of 5 methods) | no | keep: `contacts` covers only 3/5 (`otherContacts.list` and `.search` need this scope) | not stated on fetched page | [D-people], [P-people-get] |
| `directory.readonly` | Read | partial: `contacts` 2/4, `contacts.other.readonly` 2/4, `contacts.readonly` 2/4, `user.addresses.read` 2/4, `user.birthday.read` 2/4, `user.emails.read` 2/4, `user.gender.read` 2/4, `user.organization.read` 2/4, `user.phonenumbers.read` 2/4 (of 4 methods) | no (needs a Workspace account; inference from the scope text) | keep: no other scope covers `directory` methods fully | not stated on fetched page | [D-people], [P-people-get] |
| `user.addresses.read` | Read | full: all 8 other People scopes incl. `contacts`, `contacts.readonly`, `contacts.other.readonly`, `directory.readonly` 2/2 (only `people.get`, `people.getBatchGet`) | no | drop: rare. Method-level cover by `contacts` is 2/2, but these scopes probably gate profile fields on `people/me` (inference; see open questions) | not stated on fetched page | [D-people], [P-people-get], [P-people-person] |
| `user.birthday.read` | Read | full: all 8 other People scopes incl. `contacts`, `contacts.readonly`, `contacts.other.readonly`, `directory.readonly` 2/2 (only `people.get`, `people.getBatchGet`) | no | drop: rare (same note as `user.addresses.read`) | not stated on fetched page | [D-people], [P-people-get] |
| `user.emails.read` | Read | full: all 8 other People scopes incl. `contacts`, `contacts.readonly`, `contacts.other.readonly`, `directory.readonly` 2/2 (only `people.get`, `people.getBatchGet`) | no | drop: rare (same note) | not stated on fetched page | [D-people], [P-people-get] |
| `user.gender.read` | Read | full: all 8 other People scopes incl. `contacts`, `contacts.readonly`, `contacts.other.readonly`, `directory.readonly` 2/2 (only `people.get`, `people.getBatchGet`) | no | drop: rare (same note) | not stated on fetched page | [D-people], [P-people-get] |
| `user.organization.read` | Read | full: all 8 other People scopes incl. `contacts`, `contacts.readonly`, `contacts.other.readonly`, `directory.readonly` 2/2 (only `people.get`, `people.getBatchGet`) | no | drop: rare (same note) | not stated on fetched page | [D-people], [P-people-get] |
| `user.phonenumbers.read` | Read | full: all 8 other People scopes incl. `contacts`, `contacts.readonly`, `contacts.other.readonly`, `directory.readonly` 2/2 (only `people.get`, `people.getBatchGet`) | no | drop: rare (same note) | not stated on fetched page | [D-people], [P-people-get] |
| `contacts` | Write | none (of 20 methods) | no | keep: also covers `contacts.readonly` | not stated on fetched page | [D-people], [P-people-get] |

### 2.12 Admin Reports

Discovery: [D-reports] r20261117. Pages: [P-reports-qs].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `admin.reports.audit.readonly` | Read | none (of 3 methods) | yes | move: admin | not stated on fetched page | [D-reports], [P-reports-qs] |
| `admin.reports.usage.readonly` | Read | none (of 3 methods) | yes | move: admin | not stated on fetched page | [D-reports], [P-reports-qs] |

### 2.13 Admin Directory

Discovery: [D-directory] v1 r20261117. Pages: [P-dir-qs].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `admin.directory.user.readonly` | Read | full: `admin.directory.user` 7/7 (of 7 methods) | mostly: admin-only except the public view | keep (already dropped when `admin.directory.user` is requested). Non-admins can read the domain-wide public view with `viewType=domain_public`; People `directory.readonly` may serve that use (inference) | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.user.alias.readonly` | Read | full: `admin.directory.user` 3/3, `admin.directory.user.alias` 3/3, `admin.directory.user.readonly` 3/3 (of 3 methods) | yes | drop: covered by `admin.directory.user.readonly` (3/3) or `admin.directory.user` | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.orgunit.readonly` | Read | full: `admin.directory.orgunit` 2/2 (of 2 methods) | yes | keep (already dropped when `admin.directory.orgunit` is requested) | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.domain.readonly` | Read | full: `admin.directory.domain` 4/4 (of 4 methods) | yes | keep (already dropped when `admin.directory.domain` is requested) | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.customer.readonly` | Read | full: `admin.directory.customer` 1/1 (of 1 method) | yes | keep (already dropped when `admin.directory.customer` is requested) | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.userschema.readonly` | Read | full: `admin.directory.userschema` 2/2 (of 2 methods) | yes | keep (already dropped when `admin.directory.userschema` is requested) | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.rolemanagement.readonly` | Read | full: `admin.directory.rolemanagement` 5/5 (of 5 methods) | yes | keep (already dropped when `admin.directory.rolemanagement` is requested) | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.resource.calendar.readonly` | Read | full: `admin.directory.resource.calendar` 6/6 (of 6 methods) | yes | keep (already dropped when `admin.directory.resource.calendar` is requested) | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.device.mobile.readonly` | Read | full: `admin.directory.device.mobile` 2/2, `admin.directory.device.mobile.action` 2/2 (of 2 methods) | yes | keep (already dropped when `admin.directory.device.mobile` is requested) | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.device.chromeos.readonly` | Read | full: `admin.directory.device.chromeos` 4/4 (of 4 methods) | yes | keep (already dropped when `admin.directory.device.chromeos` is requested) | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.chrome.printers.readonly` | Read | full: `admin.chrome.printers` 5/5 (of 5 methods) | yes | keep (already dropped when `admin.chrome.printers` is requested) | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.user` | Write | none (of 19 methods) | mostly: admin-only except the public view | move: admin. The domain-wide public view of `users.get` and `users.list` is open to non-admins | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.user.alias` | Write | full: `admin.directory.user` 5/5; partial: `admin.directory.user.alias.readonly` 3/5, `admin.directory.user.readonly` 3/5 (of 5 methods) | yes | drop: covered by `admin.directory.user` (5/5) | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.user.security` | Write | none (of 11 methods) | yes | move: admin (no other scope covers its 11 methods) | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.orgunit` | Write | none (of 6 methods) | yes | move: admin | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.domain` | Write | partial: `admin.directory.domain.readonly` 4/8 (of 8 methods) | yes | move: admin | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.customer` | Write | none (of 3 methods) | yes | move: admin | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.userschema` | Write | none (of 6 methods) | yes | move: admin | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.rolemanagement` | Write | none (of 11 methods) | yes | move: admin | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.resource.calendar` | Write | none (of 19 methods) | yes | move: admin | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.device.mobile` | Write | partial: `admin.directory.device.mobile.action` 3/4, `admin.directory.device.mobile.readonly` 2/4 (of 4 methods) | yes | move: admin | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.device.mobile.action` | Write | full: `admin.directory.device.mobile` 3/3; partial: `admin.directory.device.mobile.readonly` 2/3 (of 3 methods) | yes | drop: covered by `admin.directory.device.mobile` (3/3) | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.device.chromeos` | Write | none (of 10 methods) | yes | move: admin | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.chrome.printers` | Write | none (of 15 methods) | yes | move: admin | not stated on fetched page | [D-directory], [P-dir-qs] |

### 2.14 Admin Groups (Directory API)

Discovery: [D-directory] v1 r20261117. Pages: [P-dir-qs].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `admin.directory.group.readonly` | Read | full: `admin.directory.group` 6/6; partial: `admin.directory.group.member` 3/6, `admin.directory.group.member.readonly` 3/6 (of 6 methods) | yes | keep (already dropped when `admin.directory.group` is requested) | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.group.member.readonly` | Read | full: `admin.directory.group` 3/3, `admin.directory.group.member` 3/3, `admin.directory.group.readonly` 3/3 (of 3 methods) | yes | drop: covered by `admin.directory.group.readonly` (3/3) or `admin.directory.group` | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.group` | Write | none (of 16 methods) | yes | move: admin | not stated on fetched page | [D-directory], [P-dir-qs] |
| `admin.directory.group.member` | Write | full: `admin.directory.group` 7/7 (of 7 methods) | yes | drop: covered by `admin.directory.group` (7/7) | not stated on fetched page | [D-directory], [P-dir-qs] |

### 2.15 Cloud Identity

Discovery: [D-cloudidentity] v1 r20261118. Pages: none fetched.

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `cloud-identity.groups.readonly` | Read | full: `cloud-identity.groups` 13/13 (of 13 methods) | no: description says "groups that you can access" | keep (already dropped when `cloud-identity.groups` is requested). Overlaps in purpose with `admin.directory.group*` (inference) | not stated on fetched page | [D-cloudidentity] |
| `cloud-identity.policies.readonly` | Read | full: `cloud-identity.policies` 2/2 (of 2 methods) | likely (inference: org-level policies) | keep (already dropped when `cloud-identity.policies` is requested) | not stated on fetched page | [D-cloudidentity] |
| `cloud-identity.inboundsso.readonly` | Read | full: `cloud-identity.inboundsso` 8/8 (of 8 methods) | likely (inference: org-level SSO) | keep (already dropped when `cloud-identity.inboundsso` is requested) | not stated on fetched page | [D-cloudidentity] |
| `cloud-identity.allowlisteddomains.readonly` | Read | full: `cloud-identity.allowlisteddomains` 2/2 (of 2 methods) | likely (inference: org-level) | keep (already dropped when `cloud-identity.allowlisteddomains` is requested) | not stated on fetched page | [D-cloudidentity] |
| `cloud-identity.devices.lookup` | Read | none (of 1 method) | no: method "must be called with end-user credentials" | drop: rare. 1 method: `devices.deviceUsers.lookup` | not stated on fetched page | [D-cloudidentity] |
| `cloud-identity.groups` | Write | partial: `cloud-identity.groups.readonly` 13/20 (of 20 methods) | no: "groups that you can access" | keep, in the admin sign-in (the UI already treats Cloud Identity as admin). Optional: drop as overlap with `admin.directory.group` (inference) | not stated on fetched page | [D-cloudidentity] |
| `cloud-identity.policies` | Write | none (of 5 methods) | likely (inference) | move: admin | not stated on fetched page | [D-cloudidentity] |
| `cloud-identity.inboundsso` | Write | none (of 19 methods) | likely (inference) | move: admin | not stated on fetched page | [D-cloudidentity] |
| `cloud-identity.allowlisteddomains` | Write | partial: `cloud-identity.allowlisteddomains.readonly` 2/4 (of 4 methods) | likely (inference) | move: admin | not stated on fetched page | [D-cloudidentity] |

### 2.16 YouTube

Discovery: [D-youtube] v3 r20261119, [D-ytanalytics] v2 r20261118, [D-ytreporting] v1 r20261118. Pages: none fetched.

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `youtube.readonly` | Read | partial: `youtube` 30/31 (of 31 methods) | no | keep (already dropped when `youtube` is requested; 30/31, only `youtube.tests.insert` is not covered) | not stated on fetched page | [D-youtube] |
| `youtube` | Write | none (of 71 methods) | no | keep | not stated on fetched page | [D-youtube] |

### 2.17 Google Ads

Discovery: none (no Discovery doc). Pages: [P-ads-rest-auth].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `adwords` | Read+Write | no scopes in Discovery (single scope per Google page) | no | keep: the only scope for the Google Ads API; no Discovery doc, so no coverage data | not stated on fetched page | [P-ads-rest-auth] |

### 2.18 Google Analytics

Discovery: [D-analyticsadmin] v1beta r20261118, [D-analyticsdata] v1beta r20261117. Pages: none fetched.

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `analytics.readonly` | Read | partial: `analytics.edit` 23/34 (of 34 methods) | no | keep. Conditional: `analytics.edit` covers 23/23 Admin API methods but 0/11 Data API methods. Drop it only if the Data API is never called (see open questions) | not stated on fetched page | [D-analyticsadmin], [D-analyticsdata], [R-business] |
| `analytics.edit` | Write | none (of 55 methods) | no | keep | not stated on fetched page | [D-analyticsadmin] |

### 2.19 Search Console

Discovery: [D-searchconsole] v1 r20261118. Pages: [P-sc-auth].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `webmasters.readonly` | Read | full: `webmasters` 6/6 (of 6 methods) | no | keep (already dropped when `webmasters` is requested; 6/6) | not stated on fetched page | [D-searchconsole], [P-sc-auth] |
| `webmasters` | Write | partial: `webmasters.readonly` 6/10 (of 10 methods) | no | keep | not stated on fetched page | [D-searchconsole], [P-sc-auth] |

### 2.20 Business Profile

Discovery: [D-mybusiness] (no scopes listed). Pages: [P-bp-oauth].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `business.manage` | Read+Write | no scopes in Discovery (single scope per Google page) | no | keep: the only current scope (the other, `plus.business.manage`, is deprecated); Discovery lists no scopes for these APIs | not stated on fetched page | [P-bp-oauth] |

### 2.21 Merchant Center

Discovery: [D-content] v2.1 r20261118, [D-merchantapi] (13 docs, r20261118). Pages: [P-shop-auth].

| scope | level | covered by (method coverage) | admin-only? | recommendation | class (info) | source |
|---|---|---|---|---|---|---|
| `content` | Read+Write | none (of 250 methods) | no | keep: the only scope in the Content API v2.1 and in all 13 Merchant API Discovery docs | not stated on fetched page | [D-content], [D-merchantapi], [P-shop-auth] |

## 3. Method

1. **Catalogue.** I parsed `level_scopes` and `business_level_scopes` from the working-tree `crates/core/src/scopes.rs`. That file has uncommitted changes (see `git status`). It lists 107 distinct scopes. The `READ_ROWS` and `WRITE_ROWS` tables in `crates/core/tests/scopes.rs` were not parsed. I did not compare them with `scopes.rs` [R-scopes].
2. **Discovery docs.** For each service I downloaded the Discovery doc listed in its table heading on 2026-11-21 and read every method's `scopes` list (all resources, all nesting levels). I used the stable version that the app or `gws` uses: Gmail v1, Calendar v3, Chat v1, Meet v2, Drive v3, Docs v1, Sheets v4, Slides v1, Forms v1, Tasks v1, People v1, Admin Reports v1, Admin Directory v1, Cloud Identity v1, YouTube v3 with YouTube Analytics v2 and YouTube Reporting v1, Analytics Admin v1beta with Analytics Data v1beta, Search Console v1, Content API v2.1 with the 13 Merchant API docs. Revisions are in the Sources section. I did not use Drive v2, Analytics Admin v1alpha, Cloud Identity v1beta1 or Workspace Events. These versions are not in the catalogue, and `business.rs:121-140` routes Analytics only to `analyticsadmin` v1beta and Merchant Center to Content API v2.1 [R-business]. Choosing the stable versions is my assumption.
3. **Coverage.** For two catalogue scopes A and B, coverage(A, B) = (methods that accept both) / (methods that accept B). I counted only the API docs of B's own service. "Full" means the ratio is 1. All methods that list scopes are counted. Ten methods in the docs I used list no scopes (5 Cloud Identity, 4 YouTube `thirdPartyLinks`, 1 Search Console `mobileFriendlyTest`). I ignored them. Excluding `channels.stop` methods changes only some denominators, not any full result.
4. **No Discovery scopes.** The Discovery docs for Business Profile (`mybusiness*`, `businessprofileperformance`) list no scopes. Google Ads has no Discovery doc. For these I used Google pages ([P-bp-oauth], [P-ads-rest-auth]). Each has one scope, so there is nothing to reduce.
5. **Display filter.** The "covered by" column shows full coverage and any partial coverage of at least half of the methods. Lower partial values are in the raw data only.
6. **Admin and sign-in limits.** I read method descriptions for service-account-only notes. Only Gmail has them (8 methods). A keyword scan for "admin" over Discovery text gave many false hits (Chat `useAdminAccess` text), so I did not use it as evidence. For the Admin SDK and Reports I cite the Google quickstart prerequisite "A Google Account in that domain with administrator privileges" [P-dir-qs], [P-reports-qs].
7. **Counts.** I re-implemented `scopes_for` (union, then drop `X.readonly` when `X` is present) and checked it against the brief: 39 for the 10-service Full selection. Then I applied each rule. Writes are not supported for Admin Reports, as in `write_supported` [R-scopes].
8. **Sensitivity classes.** Taken only from tables on pages I fetched: Gmail, Chat, Drive, Docs, Sheets, Slides, Meet. Other pages state no class per scope.
9. **Scripts.** Scripts and raw downloads are in the session scratchpad under `research/`. They are not part of the repo.

## 4. Open questions and things I could not verify

1. **Is the 50-scope limit per request or per accumulated grant?** Answered after this note was drafted: **per request**. In the bisect on 2026-11-21, a 35-scope request and then a 28-scope request both passed, which left 60 scopes granted to the client on the account. Then one 60-scope request still failed, and later requests of 50 scopes passed. Accumulated grants do not count toward the limit (measured, bisect log in the session scratchpad).
2. **Does `gws` support two separate logins** (two credential sets, or one merged grant)? Partly answered: gws v0.22.5 keeps one credential set per config folder, and its auth URL never sends `include_granted_scopes`. Source: the yup-oauth2 12.1.2 `build_authentication_request_url` and gws `build_proxy_auth_url` in `crates/google-workspace-cli/src/auth_commands.rs`, with no `include_granted_scopes` anywhere in either source tree. So a second `gws auth login` in the same folder replaces the refresh token with one that holds only the second request's scopes. Two sign-ins therefore need two config folders (for example a separate admin folder), each with its own token. Not tested end to end.
3. **People `user.*.read`.** Method coverage is 2/2 by `contacts`. I found no Google page that says which fields need these scopes. Test: call `people.get people/me` with each `personFields` value, once with `contacts` only and once with the matching `user.*` scope.
4. **Token-level check of `drive` for Docs, Sheets, Slides and Forms.** Discovery and (for Docs, Sheets, Slides) Google pages say `drive` is accepted. The Forms and Admin SDK pairs rest on Discovery only. Run one smoke call per API with a token that holds only `drive` (or only `admin.directory.user`).
5. **Chat admin mode.** Discovery lists `chat.spaces` on admin methods, the Chat table does not. I treated the Chat table as right. I did not test a call with `useAdminAccess=true` and only `chat.spaces`.
6. **Analytics Data API.** `analytics.readonly` stays because the Data API (reports) accepts `analytics.readonly` but not `analytics.edit`. `business.rs` calls only `analyticsadmin` v1beta, where `analytics.edit` covers `analytics.readonly` 23/23. The UI text says "informes", and the README enables the Data API. If the app never calls the Data API, `analytics.readonly` can go when Write is selected (1 more scope). I counted it as kept [R-business].
7. **Meet `created`.** I kept all three Meet scopes because of the Meet guide text. I did not test whether `created` can read spaces that the app did not create.
8. **Baseline 69 versus "about 67".** My count comes from the working-tree `scopes.rs`. I did not find the cause of the difference of 2.
9. **Discovery can hide conditions.** Discovery lists accepted scopes per method. It does not state admin mode (Chat) or service-account-only use (Gmail). I found the Gmail case in method text. Other APIs may have similar limits that I did not detect.
10. **Sensitivity classes** are not stated on a fetched page for Calendar, Forms, Tasks, People, Admin SDK, Cloud Identity and the business APIs. The Chat page does not list `chat.spaces.pins` or `chat.users.availability` at all (they appear only in Discovery and in the method table for pins) [P-chat-authz], [D-chat].
11. **Not evaluated.** Gmail has newer narrow scopes in Discovery (`gmail.settings.filters`, `gmail.messages.*`, `gmail.labels` and more) [D-gmail]. The catalogue does not use them. They could reduce the sensitivity class but not the count, so I left them out.
12. **Cloud Identity admin status.** `cloud-identity.groups` ("groups that you can access") and `devices.lookup` ("end-user credentials") are not admin-only per Discovery, but the UI treats the whole row as admin. I marked `policies`, `inboundsso` and `allowlisteddomains` as likely admin-only. This is inference.

## Sources

Tags used above. All pages and Discovery docs were fetched on 2026-11-21. Treated as data.

Discovery docs (revision in parentheses). Directory of all docs: https://www.googleapis.com/discovery/v1/apis

- `D-gmail` (r20261116): https://gmail.googleapis.com/$discovery/rest?version=v1
- `D-calendar` (r20261118): https://calendar-json.googleapis.com/$discovery/rest?version=v3
- `D-chat` (r20261120): https://chat.googleapis.com/$discovery/rest?version=v1
- `D-meet` (r20261117): https://meet.googleapis.com/$discovery/rest?version=v2
- `D-drive` (v3) (r20261117): https://www.googleapis.com/discovery/v1/apis/drive/v3/rest
- `D-docs` (r20261118): https://docs.googleapis.com/$discovery/rest?version=v1
- `D-sheets` (r20261118): https://sheets.googleapis.com/$discovery/rest?version=v4
- `D-slides` (r20261118): https://slides.googleapis.com/$discovery/rest?version=v1
- `D-forms` (r20261115): https://forms.googleapis.com/$discovery/rest?version=v1
- `D-tasks` (r20261117): https://tasks.googleapis.com/$discovery/rest?version=v1
- `D-people` (r20261118): https://people.googleapis.com/$discovery/rest?version=v1
- `D-reports` (Admin SDK reports_v1) (r20261117): https://admin.googleapis.com/$discovery/rest?version=reports_v1
- `D-directory` (Admin SDK directory_v1) (r20261117): https://admin.googleapis.com/$discovery/rest?version=directory_v1
- `D-cloudidentity` (v1) (r20261118): https://cloudidentity.googleapis.com/$discovery/rest?version=v1
- `D-youtube` (v3) (r20261119): https://youtube.googleapis.com/$discovery/rest?version=v3
- `D-ytanalytics` (v2) (r20261118): https://youtubeanalytics.googleapis.com/$discovery/rest?version=v2
- `D-ytreporting` (v1) (r20261118): https://youtubereporting.googleapis.com/$discovery/rest?version=v1
- `D-analyticsadmin` (v1beta) (r20261118): https://analyticsadmin.googleapis.com/$discovery/rest?version=v1beta
- `D-analyticsdata` (v1beta) (r20261117): https://analyticsdata.googleapis.com/$discovery/rest?version=v1beta
- `D-searchconsole` (v1) (r20261118): https://searchconsole.googleapis.com/$discovery/rest?version=v1
- `D-content` (v2.1) (r20261118): https://www.googleapis.com/discovery/v1/apis/content/v2.1/rest
- `D-merchantapi` (13 docs, all (r20261118); example): https://merchantapi.googleapis.com/$discovery/rest?version=products_v1
- `D-mybusiness` (no scopes listed), example: https://mybusinessbusinessinformation.googleapis.com/$discovery/rest?version=v1

Google pages:

- `P-gmail-scopes`: https://developers.google.com/workspace/gmail/api/auth/scopes
- `P-chat-authz`: https://developers.google.com/workspace/chat/authenticate-authorize
- `P-drive-scopes`: https://developers.google.com/workspace/drive/api/guides/api-specific-auth
- `P-docs-auth`: https://developers.google.com/workspace/docs/api/auth
- `P-sheets-scopes`: https://developers.google.com/workspace/sheets/api/scopes
- `P-slides-scopes`: https://developers.google.com/workspace/slides/api/scopes
- `P-meet-scopes`: https://developers.google.com/workspace/meet/api/guides/authenticate-authorize
- `P-cal-auth`: https://developers.google.com/workspace/calendar/api/auth
- `P-people-get`: https://developers.google.com/people/api/rest/v1/people/get
- `P-people-person`: https://developers.google.com/people/api/rest/v1/people
- `P-people-source`: https://developers.google.com/people/api/rest/v1/ReadSourceType
- `P-dir-qs`: https://developers.google.com/workspace/admin/directory/v1/quickstart/python
- `P-reports-qs`: https://developers.google.com/workspace/admin/reports/v1/quickstart/python
- `P-sc-auth`: https://developers.google.com/webmaster-tools/v1/how-tos/authorizing
- `P-bp-oauth`: https://developers.google.com/my-business/content/implement-oauth
- `P-ads-rest-auth`: https://developers.google.com/google-ads/api/rest/auth
- `P-shop-auth`: https://developers.google.com/shopping-content/guides/how-tos/authorizing
- Google's OAuth scope list (checked for scope names only; it states no per-scope class): https://developers.google.com/identity/protocols/oauth2/scopes

Repository files (working tree, with uncommitted changes):

- `R-scopes`: `crates/core/src/scopes.rs`
- `R-business`: `crates/core/src/business.rs`


<!-- link definitions for the source tags -->
[D-analyticsadmin]: https://analyticsadmin.googleapis.com/$discovery/rest?version=v1beta
[D-analyticsdata]: https://analyticsdata.googleapis.com/$discovery/rest?version=v1beta
[D-calendar]: https://calendar-json.googleapis.com/$discovery/rest?version=v3
[D-chat]: https://chat.googleapis.com/$discovery/rest?version=v1
[D-cloudidentity]: https://cloudidentity.googleapis.com/$discovery/rest?version=v1
[D-content]: https://www.googleapis.com/discovery/v1/apis/content/v2.1/rest
[D-directory]: https://admin.googleapis.com/$discovery/rest?version=directory_v1
[D-docs]: https://docs.googleapis.com/$discovery/rest?version=v1
[D-drive]: https://www.googleapis.com/discovery/v1/apis/drive/v3/rest
[D-forms]: https://forms.googleapis.com/$discovery/rest?version=v1
[D-gmail]: https://gmail.googleapis.com/$discovery/rest?version=v1
[D-meet]: https://meet.googleapis.com/$discovery/rest?version=v2
[D-merchantapi]: https://merchantapi.googleapis.com/$discovery/rest?version=products_v1
[D-mybusiness]: https://mybusinessbusinessinformation.googleapis.com/$discovery/rest?version=v1
[D-people]: https://people.googleapis.com/$discovery/rest?version=v1
[D-reports]: https://admin.googleapis.com/$discovery/rest?version=reports_v1
[D-searchconsole]: https://searchconsole.googleapis.com/$discovery/rest?version=v1
[D-sheets]: https://sheets.googleapis.com/$discovery/rest?version=v4
[D-slides]: https://slides.googleapis.com/$discovery/rest?version=v1
[D-tasks]: https://tasks.googleapis.com/$discovery/rest?version=v1
[D-youtube]: https://youtube.googleapis.com/$discovery/rest?version=v3
[D-ytanalytics]: https://youtubeanalytics.googleapis.com/$discovery/rest?version=v2
[D-ytreporting]: https://youtubereporting.googleapis.com/$discovery/rest?version=v1
[P-ads-rest-auth]: https://developers.google.com/google-ads/api/rest/auth
[P-bp-oauth]: https://developers.google.com/my-business/content/implement-oauth
[P-cal-auth]: https://developers.google.com/workspace/calendar/api/auth
[P-chat-authz]: https://developers.google.com/workspace/chat/authenticate-authorize
[P-dir-qs]: https://developers.google.com/workspace/admin/directory/v1/quickstart/python
[P-docs-auth]: https://developers.google.com/workspace/docs/api/auth
[P-drive-scopes]: https://developers.google.com/workspace/drive/api/guides/api-specific-auth
[P-gmail-scopes]: https://developers.google.com/workspace/gmail/api/auth/scopes
[P-meet-scopes]: https://developers.google.com/workspace/meet/api/guides/authenticate-authorize
[P-people-get]: https://developers.google.com/people/api/rest/v1/people/get
[P-people-person]: https://developers.google.com/people/api/rest/v1/people
[P-people-source]: https://developers.google.com/people/api/rest/v1/ReadSourceType
[P-reports-qs]: https://developers.google.com/workspace/admin/reports/v1/quickstart/python
[P-sc-auth]: https://developers.google.com/webmaster-tools/v1/how-tos/authorizing
[P-sheets-scopes]: https://developers.google.com/workspace/sheets/api/scopes
[P-shop-auth]: https://developers.google.com/shopping-content/guides/how-tos/authorizing
[P-slides-scopes]: https://developers.google.com/workspace/slides/api/scopes
[R-business]: ../../crates/core/src/business.rs
[R-scopes]: ../../crates/core/src/scopes.rs
