//! Scope catalogue tests.

use haulmer_googleworkspace_mcp_core::scopes;
use haulmer_googleworkspace_mcp_core::{
    BusinessConnector, BusinessSelection, Level, Selection, Service,
};

#[test]
fn default_selection_is_read_for_gmail_calendar_chat_meet_only() {
    assert_eq!(
        scopes::default_selection(),
        Selection::from_levels([
            (Service::Gmail, Level::Read),
            (Service::Calendar, Level::Read),
            (Service::Chat, Level::Read),
            (Service::Meet, Level::Read),
        ])
    );
}

#[test]
fn normalize_adds_read_for_every_write_and_keeps_the_rest() {
    let selection = Selection::from_levels([
        (Service::Gmail, Level::Write),
        (Service::Drive, Level::Write),
        (Service::Chat, Level::Read),
    ]);
    assert_eq!(
        scopes::normalize(&selection),
        Selection::from_levels([
            (Service::Gmail, Level::Read),
            (Service::Gmail, Level::Write),
            (Service::Drive, Level::Read),
            (Service::Drive, Level::Write),
            (Service::Chat, Level::Read),
        ])
    );
}

/// Full URL for a short scope name, written out so expectations stay independent of the code.
fn url(short: &str) -> String {
    if short.starts_with("https://") {
        short.to_string()
    } else {
        format!("https://www.googleapis.com/auth/{short}")
    }
}

/// `scopes_for` as a sorted list, after checking it has no duplicates.
fn sorted_scopes(selection: &Selection) -> Vec<String> {
    let mut urls = scopes::scopes_for(selection);
    let count = urls.len();
    urls.sort();
    urls.dedup();
    assert_eq!(urls.len(), count, "duplicate scopes in {urls:?}");
    urls
}

fn sorted(shorts: &[&str]) -> Vec<String> {
    let mut urls: Vec<String> = shorts.iter().map(|short| url(short)).collect();
    urls.sort();
    urls
}

/// Every read-only user-facing scope of each service's Google APIs.
const READ_ROWS: [(Service, &[&str]); 15] = [
    (Service::Gmail, &["gmail.readonly"]),
    (Service::Calendar, &["calendar.readonly"]),
    (
        Service::Chat,
        &[
            "chat.spaces.readonly",
            "chat.messages.readonly",
            "chat.memberships.readonly",
            "chat.messages.reactions.readonly",
            "chat.users.readstate.readonly",
            "chat.users.sections.readonly",
            "chat.users.availability.readonly",
            "chat.spaces.pins.readonly",
            "chat.customemojis.readonly",
            "chat.admin.spaces.readonly",
            "chat.admin.memberships.readonly",
        ],
    ),
    (Service::Meet, &["meetings.space.readonly"]),
    (Service::Drive, &["drive.readonly", "drive.apps.readonly"]),
    (Service::Docs, &["documents.readonly"]),
    (Service::Sheets, &["spreadsheets.readonly"]),
    (Service::Slides, &["presentations.readonly"]),
    (
        Service::Forms,
        &["forms.body.readonly", "forms.responses.readonly"],
    ),
    (Service::Tasks, &["tasks.readonly"]),
    (
        Service::People,
        &[
            "contacts.readonly",
            "contacts.other.readonly",
            "directory.readonly",
            "user.addresses.read",
            "user.birthday.read",
            "user.emails.read",
            "user.gender.read",
            "user.organization.read",
            "user.phonenumbers.read",
        ],
    ),
    (
        Service::AdminReports,
        &[
            "admin.reports.audit.readonly",
            "admin.reports.usage.readonly",
        ],
    ),
    (
        Service::AdminDirectory,
        &[
            "admin.directory.user.readonly",
            "admin.directory.user.alias.readonly",
            "admin.directory.orgunit.readonly",
            "admin.directory.domain.readonly",
            "admin.directory.customer.readonly",
            "admin.directory.userschema.readonly",
            "admin.directory.rolemanagement.readonly",
            "admin.directory.resource.calendar.readonly",
            "admin.directory.device.mobile.readonly",
            "admin.directory.device.chromeos.readonly",
            "admin.chrome.printers.readonly",
        ],
    ),
    (
        Service::AdminGroups,
        &[
            "admin.directory.group.readonly",
            "admin.directory.group.member.readonly",
        ],
    ),
    (
        Service::CloudIdentity,
        &[
            "cloud-identity.groups.readonly",
            "cloud-identity.policies.readonly",
            "cloud-identity.inboundsso.readonly",
            "cloud-identity.allowlisteddomains.readonly",
            "cloud-identity.devices.lookup",
        ],
    ),
];

/// Scopes added by Full access, on top of the Read scopes; broader scopes already cover
/// narrower ones (`drive` covers `drive.file`, `mail.google.com` covers `gmail.modify`).
const WRITE_ROWS: [(Service, &[&str]); 14] = [
    (
        Service::Gmail,
        &[
            "https://mail.google.com/",
            "gmail.settings.basic",
            "gmail.settings.sharing",
        ],
    ),
    (Service::Calendar, &["calendar"]),
    (
        Service::Chat,
        &[
            "chat.spaces",
            "chat.memberships",
            "chat.messages",
            "chat.messages.reactions",
            "chat.users.readstate",
            "chat.users.sections",
            "chat.users.availability",
            "chat.users.spacesettings",
            "chat.spaces.pins",
            "chat.customemojis",
            "chat.delete",
            "chat.import",
            "chat.admin.spaces",
            "chat.admin.memberships",
            "chat.admin.delete",
        ],
    ),
    (
        Service::Meet,
        &["meetings.space.created", "meetings.space.settings"],
    ),
    (Service::Drive, &["drive", "drive.scripts"]),
    (Service::Docs, &["documents"]),
    (Service::Sheets, &["spreadsheets"]),
    (Service::Slides, &["presentations"]),
    (Service::Forms, &["forms.body"]),
    (Service::Tasks, &["tasks"]),
    (Service::People, &["contacts"]),
    (
        Service::AdminDirectory,
        &[
            "admin.directory.user",
            "admin.directory.user.alias",
            "admin.directory.user.security",
            "admin.directory.orgunit",
            "admin.directory.domain",
            "admin.directory.customer",
            "admin.directory.userschema",
            "admin.directory.rolemanagement",
            "admin.directory.resource.calendar",
            "admin.directory.device.mobile",
            "admin.directory.device.mobile.action",
            "admin.directory.device.chromeos",
            "admin.chrome.printers",
        ],
    ),
    (
        Service::AdminGroups,
        &["admin.directory.group", "admin.directory.group.member"],
    ),
    (
        Service::CloudIdentity,
        &[
            "cloud-identity.groups",
            "cloud-identity.policies",
            "cloud-identity.inboundsso",
            "cloud-identity.allowlisteddomains",
        ],
    ),
];

#[test]
fn every_read_row_maps_to_exactly_its_scopes() {
    for (service, expected) in READ_ROWS {
        let selection = Selection::from_levels([(service, Level::Read)]);
        assert_eq!(
            sorted_scopes(&selection),
            sorted(expected),
            "{service:?} Read"
        );
    }
}

#[test]
fn every_write_row_maps_to_its_write_scopes_plus_its_read_scopes() {
    for (service, write) in WRITE_ROWS {
        let read = READ_ROWS.iter().find(|row| row.0 == service).unwrap().1;
        let expected: Vec<&str> = read.iter().chain(write).copied().collect();
        let selection = Selection::from_levels([(service, Level::Write)]);
        assert_eq!(
            sorted_scopes(&selection),
            sorted(&expected),
            "{service:?} Write"
        );
    }
}

#[test]
fn admin_reports_has_no_write_level() {
    let selection = Selection::from_levels([(Service::AdminReports, Level::Write)]);
    assert_eq!(
        sorted_scopes(&selection),
        sorted_scopes(&Selection::from_levels([(
            Service::AdminReports,
            Level::Read
        )]))
    );
}

#[test]
fn empty_selection_has_no_scopes() {
    assert!(scopes::scopes_for(&Selection::new()).is_empty());
}

#[test]
fn scopes_arg_is_the_full_urls_joined_with_commas() {
    let selection = Selection::from_levels([(Service::Calendar, Level::Write)]);
    let arg = scopes::scopes_arg(&selection);
    let mut parts: Vec<&str> = arg.split(',').collect();
    parts.sort();
    assert_eq!(
        parts,
        [
            "https://www.googleapis.com/auth/calendar",
            "https://www.googleapis.com/auth/calendar.readonly",
        ]
    );
    assert!(!arg.contains(' '));
}

#[test]
fn reverse_mapping_round_trips_every_normalized_selection() {
    for service in Service::ALL {
        for levels in [&[][..], &[Level::Read], &[Level::Read, Level::Write]] {
            if service == Service::AdminReports && levels.contains(&Level::Write) {
                continue;
            }
            let selection =
                Selection::from_levels(levels.iter().copied().map(|level| (service, level)));
            let granted = scopes::scopes_for(&selection);
            assert_eq!(scopes::selection_from_scopes(&granted), selection);
        }
    }

    let selection = scopes::normalize(&Selection::from_levels([
        (Service::Gmail, Level::Read),
        (Service::Calendar, Level::Write),
        (Service::Docs, Level::Write),
        (Service::AdminGroups, Level::Write),
    ]));
    let granted = scopes::scopes_for(&selection);
    assert_eq!(scopes::selection_from_scopes(&granted), selection);
}

#[test]
fn a_level_is_granted_only_when_all_its_scopes_are() {
    // Person unticked chat.memberships.readonly and gmail.modify on the consent screen.
    let granted = [
        url("chat.spaces.readonly"),
        url("chat.messages.readonly"),
        url("gmail.readonly"),
        url("gmail.send"),
    ];
    assert_eq!(
        scopes::selection_from_scopes(&granted),
        Selection::from_levels([(Service::Gmail, Level::Read)])
    );
}

#[test]
fn identity_scopes_are_ignored() {
    let granted = [
        "openid".to_string(),
        "email".to_string(),
        "profile".to_string(),
        url("userinfo.email"),
        url("userinfo.profile"),
        url("tasks.readonly"),
    ];
    assert_eq!(
        scopes::selection_from_scopes(&granted),
        Selection::from_levels([(Service::Tasks, Level::Read)])
    );
    assert!(scopes::selection_from_scopes(&granted[..5]).is_empty());
}

#[test]
fn catalogue_lists_every_service_in_ui_order_with_plain_labels() {
    let catalogue = scopes::catalogue();
    let services: Vec<Service> = catalogue.iter().map(|info| info.service).collect();
    assert_eq!(
        services,
        [
            Service::Gmail,
            Service::Calendar,
            Service::Chat,
            Service::Meet,
            Service::Drive,
            Service::Docs,
            Service::Sheets,
            Service::Slides,
            Service::Forms,
            Service::Tasks,
            Service::People,
            Service::AdminReports,
            Service::AdminDirectory,
            Service::AdminGroups,
            Service::CloudIdentity,
        ]
    );
    let names: Vec<&str> = catalogue.iter().map(|info| info.name).collect();
    assert_eq!(
        names,
        [
            "Gmail",
            "Calendar",
            "Chat",
            "Meet",
            "Drive",
            "Docs",
            "Sheets",
            "Slides",
            "Forms",
            "Tasks",
            "People",
            "Informes de administración",
            "Directorio de administración",
            "Grupos de administración",
            "Cloud Identity",
        ]
    );
    assert_eq!(catalogue[0].read, "Leer correos");
    assert_eq!(catalogue[0].write, "Acceso completo al correo");
    for info in &catalogue {
        assert!(!info.read.is_empty(), "{info:?}");
        assert!(!info.write.is_empty(), "{info:?}");
    }

    assert!(catalogue[..11].iter().all(|info| info.enabled));
    assert!(
        catalogue[11..]
            .iter()
            .filter(|info| info.service != Service::AdminDirectory)
            .all(|info| !info.enabled)
    );
    assert!(
        catalogue[11..]
            .iter()
            .filter(|info| info.service == Service::AdminReports)
            .all(|info| !info.write_enabled)
    );
    assert!(
        catalogue
            .iter()
            .find(|info| info.service == Service::AdminDirectory)
            .unwrap()
            .write_enabled
    );
    assert!(
        catalogue
            .iter()
            .find(|info| info.service == Service::AdminGroups)
            .unwrap()
            .write_enabled
    );
    assert!(
        catalogue
            .iter()
            .find(|info| info.service == Service::CloudIdentity)
            .unwrap()
            .write_enabled
    );

    let admin_granted = Selection::from_levels([(Service::AdminReports, Level::Read)]);
    let with_admin = scopes::catalogue_for(&admin_granted);
    assert!(
        with_admin
            .iter()
            .find(|info| info.service == Service::AdminReports)
            .unwrap()
            .enabled
    );
}

#[test]
fn business_selection_maps_youtube_read_scope() {
    let selection = BusinessSelection::from_levels([(BusinessConnector::Youtube, Level::Read)]);
    assert_eq!(
        scopes::business_scopes_arg(&selection),
        "https://www.googleapis.com/auth/youtube.readonly"
    );
    assert_eq!(
        scopes::business_selection_from_scopes(&[
            "https://www.googleapis.com/auth/youtube.readonly".into()
        ]),
        selection
    );
}

#[test]
fn every_business_connector_maps_read_and_full_access_scopes() {
    let rows: [(BusinessConnector, &[&str], &[&str]); 6] = [
        (
            BusinessConnector::Youtube,
            &["youtube.readonly"],
            &["youtube.readonly", "youtube"],
        ),
        (BusinessConnector::GoogleAds, &["adwords"], &["adwords"]),
        (
            BusinessConnector::Analytics,
            &["analytics.readonly"],
            &["analytics.readonly", "analytics.edit"],
        ),
        (
            BusinessConnector::SearchConsole,
            &["webmasters.readonly"],
            &["webmasters.readonly", "webmasters"],
        ),
        (
            BusinessConnector::BusinessProfile,
            &["business.manage"],
            &["business.manage"],
        ),
        (
            BusinessConnector::MerchantCenter,
            &["content"],
            &["content"],
        ),
    ];

    for (connector, read, full) in rows {
        assert_eq!(
            sorted_business_scopes(&BusinessSelection::from_levels([(connector, Level::Read)])),
            sorted(read),
            "{connector:?} Read"
        );
        assert_eq!(
            sorted_business_scopes(&BusinessSelection::from_levels([(connector, Level::Write)])),
            sorted(full),
            "{connector:?} Write"
        );
    }
}

fn sorted_business_scopes(selection: &BusinessSelection) -> Vec<String> {
    let mut scopes = haulmer_googleworkspace_mcp_core::scopes::business_scopes_for(selection);
    scopes.sort();
    scopes
}
