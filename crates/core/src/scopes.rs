//! T02 — scope catalogue: selection <-> Google scopes, defaults and UI labels.
//! Scope catalogue for Workspace and business connectors.

use std::collections::HashSet;

use crate::{
    BusinessConnector, BusinessConnectorInfo, BusinessSelection, Level, Selection, Service,
    ServiceCategory, ServiceInfo,
};

/// Prefix of every Google scope URL in the catalogue.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::scopes::SCOPE_PREFIX;
/// assert_eq!(format!("{SCOPE_PREFIX}drive"), "https://www.googleapis.com/auth/drive");
/// ```
pub const SCOPE_PREFIX: &str = "https://www.googleapis.com/auth/";

/// Most scopes one `gws auth login` may request. Google's consent screen fails with an
/// internal error (RPC status 13, no redirect) when one request has more than 50 scopes
/// (measured: 50 passes, 51 fails), and gws always adds `openid`, `userinfo.email` and
/// `userinfo.profile`.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::scopes::MAX_LOGIN_SCOPES;
/// assert_eq!(MAX_LOGIN_SCOPES, 47);
/// ```
pub const MAX_LOGIN_SCOPES: usize = 50 - 3;

/// Default selection: Read for the common Workspace services; optional services and
/// administrative capabilities are off by default.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::{scopes, Level, Service};
/// assert!(scopes::default_selection().contains(Service::Gmail, Level::Read));
/// ```
pub fn default_selection() -> Selection {
    Selection::from_levels(
        [
            Service::Gmail,
            Service::Calendar,
            Service::Chat,
            Service::Meet,
        ]
        .map(|service| (service, Level::Read)),
    )
}

/// Whether the catalogue can safely request a Write level for this service.
fn write_supported(service: Service) -> bool {
    !matches!(service, Service::AdminReports)
}

/// Normalises a selection: Write implies Read for the same service.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::{scopes, Level, Selection, Service};
/// let normalized = scopes::normalize(&Selection::from_levels([(Service::Gmail, Level::Write)]));
/// assert!(normalized.contains(Service::Gmail, Level::Read));
/// ```
pub fn normalize(selection: &Selection) -> Selection {
    let mut normalized = selection.clone();
    for level in selection.iter().filter(|level| level.level == Level::Write) {
        normalized.insert(level.service, Level::Read);
    }
    normalized
}

/// Full scope URLs for a selection (normalised first), without duplicates. A read-only
/// scope is left out when its broader scope is also requested (`calendar` covers
/// `calendar.readonly`).
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::{scopes, Level, Selection, Service};
/// let urls = scopes::scopes_for(&Selection::from_levels([(Service::Tasks, Level::Read)]));
/// assert_eq!(urls, ["https://www.googleapis.com/auth/tasks.readonly"]);
/// let urls = scopes::scopes_for(&Selection::from_levels([(Service::Tasks, Level::Write)]));
/// assert_eq!(urls, ["https://www.googleapis.com/auth/tasks"]);
/// ```
pub fn scopes_for(selection: &Selection) -> Vec<String> {
    let mut seen = HashSet::new();
    without_covered_scopes(
        normalize(selection)
            .iter()
            .filter(|level| level.level == Level::Read || write_supported(level.service))
            .flat_map(|level| level_scopes(level.service, level.level))
            .map(scope_url)
            .filter(|scope| seen.insert(scope.clone()))
            .collect(),
    )
}

/// Every scope one login requests: the Workspace scopes, then the business connector scopes.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::{
///     scopes, BusinessConnector, BusinessSelection, Level, Selection, Service,
/// };
/// let urls = scopes::login_scopes(
///     &Selection::from_levels([(Service::Tasks, Level::Read)]),
///     &BusinessSelection::from_levels([(BusinessConnector::Youtube, Level::Read)]),
/// );
/// assert_eq!(
///     urls,
///     [
///         "https://www.googleapis.com/auth/tasks.readonly",
///         "https://www.googleapis.com/auth/youtube.readonly",
///     ]
/// );
/// ```
pub fn login_scopes(selection: &Selection, business_selection: &BusinessSelection) -> Vec<String> {
    let mut scopes = scopes_for(selection);
    scopes.extend(business_scopes_for(business_selection));
    scopes
}

/// Leaves out each scope whose [`broader_scope`] is in the same list, so the login stays
/// under [`MAX_LOGIN_SCOPES`].
///
/// # Example
///
/// ```ignore
/// let urls = without_covered_scopes(vec![url("calendar.readonly"), url("calendar")]);
/// assert_eq!(urls, [url("calendar")]);
/// ```
fn without_covered_scopes(scopes: Vec<String>) -> Vec<String> {
    let requested: HashSet<String> = scopes.iter().cloned().collect();
    scopes
        .into_iter()
        .filter(|scope| broader_scope(scope).is_none_or(|broader| !requested.contains(broader)))
        .collect()
}

/// The broader scope that every Google API method accepting `scope` also accepts: `X` for
/// `X.readonly`, and `https://mail.google.com/` for `gmail.readonly`
/// (docs/research/oauth-scope-reduction.md). A login that requests the broader scope leaves
/// `scope` out, and status counts `scope` as granted when the broader one is.
///
/// # Example
///
/// ```ignore
/// assert_eq!(broader_scope(&url("calendar.readonly")), Some(url("calendar").as_str()));
/// assert_eq!(broader_scope(&url("gmail.readonly")), Some("https://mail.google.com/"));
/// ```
fn broader_scope(scope: &str) -> Option<&str> {
    if scope == format!("{SCOPE_PREFIX}gmail.readonly") {
        Some("https://mail.google.com/")
    } else {
        scope.strip_suffix(".readonly")
    }
}

/// Scope names of one service level: the spec table. Entries may be full URLs
/// when Google uses a scope outside the standard `SCOPE_PREFIX`.
///
/// # Example
///
/// ```ignore
/// assert_eq!(level_scopes(Service::Drive, Level::Write), ["drive"]);
/// ```
fn level_scopes(service: Service, level: Level) -> &'static [&'static str] {
    // Read: every read-only scope of the service's APIs. Write: every other user-facing scope
    // that a broader scope does not already cover (e.g. `drive` covers `drive.file`).
    // A scope that a broader scope of the same level covers in every API method is left out
    // (docs/research/oauth-scope-reduction.md). `gmail.settings.sharing` is left out because
    // only service accounts with domain-wide delegation can use it.
    match (service, level) {
        (Service::Gmail, Level::Read) => &["gmail.readonly"],
        (Service::Gmail, Level::Write) => &["https://mail.google.com/", "gmail.settings.basic"],
        (Service::Calendar, Level::Read) => &["calendar.readonly"],
        (Service::Calendar, Level::Write) => &["calendar"],
        // `chat.messages*` covers reactions and `chat.spaces*` covers pins.
        (Service::Chat, Level::Read) => &[
            "chat.spaces.readonly",
            "chat.messages.readonly",
            "chat.memberships.readonly",
            "chat.users.readstate.readonly",
            "chat.users.sections.readonly",
            "chat.users.availability.readonly",
            "chat.customemojis.readonly",
            "chat.admin.spaces.readonly",
            "chat.admin.memberships.readonly",
        ],
        (Service::Chat, Level::Write) => &[
            "chat.spaces",
            "chat.memberships",
            "chat.messages",
            "chat.users.readstate",
            "chat.users.sections",
            "chat.users.availability",
            "chat.users.spacesettings",
            "chat.customemojis",
            "chat.delete",
            "chat.admin.spaces",
            "chat.admin.memberships",
            "chat.admin.delete",
        ],
        (Service::Meet, Level::Read) => &["meetings.space.readonly"],
        (Service::Meet, Level::Write) => &["meetings.space.created", "meetings.space.settings"],
        (Service::Drive, Level::Read) => &["drive.readonly", "drive.apps.readonly"],
        // `drive` covers `drive.scripts`.
        (Service::Drive, Level::Write) => &["drive"],
        (Service::Docs, Level::Read) => &["documents.readonly"],
        (Service::Docs, Level::Write) => &["documents"],
        (Service::Sheets, Level::Read) => &["spreadsheets.readonly"],
        (Service::Sheets, Level::Write) => &["spreadsheets"],
        (Service::Slides, Level::Read) => &["presentations.readonly"],
        (Service::Slides, Level::Write) => &["presentations"],
        (Service::Forms, Level::Read) => &["forms.body.readonly", "forms.responses.readonly"],
        (Service::Forms, Level::Write) => &["forms.body"],
        (Service::Tasks, Level::Read) => &["tasks.readonly"],
        (Service::Tasks, Level::Write) => &["tasks"],
        (Service::People, Level::Read) => &[
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
        (Service::People, Level::Write) => &["contacts"],
        (Service::AdminReports, Level::Read) => &[
            "admin.reports.audit.readonly",
            "admin.reports.usage.readonly",
        ],
        (Service::AdminReports, Level::Write) => &["admin.reports.audit.readonly"],
        // `admin.directory.user*` covers aliases and `device.mobile` covers device actions.
        (Service::AdminDirectory, Level::Read) => &[
            "admin.directory.user.readonly",
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
        (Service::AdminDirectory, Level::Write) => &[
            "admin.directory.user",
            "admin.directory.user.security",
            "admin.directory.orgunit",
            "admin.directory.domain",
            "admin.directory.customer",
            "admin.directory.userschema",
            "admin.directory.rolemanagement",
            "admin.directory.resource.calendar",
            "admin.directory.device.mobile",
            "admin.directory.device.chromeos",
            "admin.chrome.printers",
        ],
        // `admin.directory.group*` covers members.
        (Service::AdminGroups, Level::Read) => &["admin.directory.group.readonly"],
        (Service::AdminGroups, Level::Write) => &["admin.directory.group"],
        (Service::CloudIdentity, Level::Read) => &[
            "cloud-identity.groups.readonly",
            "cloud-identity.policies.readonly",
            "cloud-identity.inboundsso.readonly",
            "cloud-identity.allowlisteddomains.readonly",
            "cloud-identity.devices.lookup",
        ],
        (Service::CloudIdentity, Level::Write) => &[
            "cloud-identity.groups",
            "cloud-identity.policies",
            "cloud-identity.inboundsso",
            "cloud-identity.allowlisteddomains",
        ],
    }
}

/// The `--scopes` argument for `gws auth login`: [`scopes_for`] joined with commas.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::{scopes, Level, Selection, Service};
/// let arg = scopes::scopes_arg(&Selection::from_levels([(Service::Calendar, Level::Read)]));
/// assert_eq!(arg, "https://www.googleapis.com/auth/calendar.readonly");
/// ```
pub fn scopes_arg(selection: &Selection) -> String {
    scopes_for(selection).join(",")
}

/// Full scope URLs for a business connector selection (normalised first), without
/// duplicates and without read-only scopes that a broader requested scope covers.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::{scopes, BusinessConnector, BusinessSelection, Level};
/// let selection = BusinessSelection::from_levels([(BusinessConnector::Youtube, Level::Write)]);
/// assert_eq!(scopes::business_scopes_for(&selection), ["https://www.googleapis.com/auth/youtube"]);
/// ```
pub fn business_scopes_for(selection: &BusinessSelection) -> Vec<String> {
    let mut seen = HashSet::new();
    without_covered_scopes(
        business_normalize(selection)
            .iter()
            .flat_map(|access| business_level_scopes(access.connector, access.level))
            .map(scope_url)
            .filter(|scope| seen.insert(scope.clone()))
            .collect(),
    )
}

/// Normalises business access so Write always includes Read.
pub fn business_normalize(selection: &BusinessSelection) -> BusinessSelection {
    let mut normalized = selection.clone();
    for access in selection
        .iter()
        .filter(|access| access.level == Level::Write)
    {
        normalized.insert(access.connector, Level::Read);
    }
    normalized
}

/// The `--scopes` argument for business connector access.
pub fn business_scopes_arg(selection: &BusinessSelection) -> String {
    business_scopes_for(selection).join(",")
}

fn business_level_scopes(connector: BusinessConnector, level: Level) -> &'static [&'static str] {
    match (connector, level) {
        (BusinessConnector::Youtube, Level::Read) => &["youtube.readonly"],
        (BusinessConnector::Youtube, Level::Write) => &["youtube"],
        (BusinessConnector::GoogleAds, _) => &["adwords"],
        (BusinessConnector::Analytics, Level::Read) => &["analytics.readonly"],
        (BusinessConnector::Analytics, Level::Write) => &["analytics.edit"],
        (BusinessConnector::SearchConsole, Level::Read) => &["webmasters.readonly"],
        (BusinessConnector::SearchConsole, Level::Write) => &["webmasters"],
        (BusinessConnector::BusinessProfile, _) => &["business.manage"],
        (BusinessConnector::MerchantCenter, _) => &["content"],
    }
}

fn scope_url(scope: &&str) -> String {
    if scope.starts_with("https://") {
        (*scope).to_string()
    } else {
        format!("{SCOPE_PREFIX}{scope}")
    }
}

/// Converts granted OAuth scopes into business connector levels.
pub fn business_selection_from_scopes(granted: &[String]) -> BusinessSelection {
    let mut selection = BusinessSelection::new();
    for connector in [
        BusinessConnector::Youtube,
        BusinessConnector::GoogleAds,
        BusinessConnector::Analytics,
        BusinessConnector::SearchConsole,
        BusinessConnector::BusinessProfile,
        BusinessConnector::MerchantCenter,
    ] {
        for level in [Level::Read, Level::Write] {
            if business_level_scopes(connector, level)
                .iter()
                .all(|scope| has_scope(granted, scope))
            {
                selection.insert(connector, level);
            }
        }
    }
    selection
}

/// Granted scopes (as `gws auth status` prints them) -> granted levels. A level counts only
/// if all its scopes are granted (a read-only scope also counts when its broader scope is
/// granted); identity scopes (`openid`, `email`, `profile`, `userinfo.email`,
/// `userinfo.profile`) are ignored.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::{scopes, Level, Service};
/// let granted = scopes::selection_from_scopes(&["https://www.googleapis.com/auth/tasks.readonly".to_string()]);
/// assert!(granted.contains(Service::Tasks, Level::Read));
/// ```
pub fn selection_from_scopes(granted: &[String]) -> Selection {
    let mut selection = Selection::new();
    for service in Service::ALL {
        for level in [Level::Read, Level::Write] {
            if level == Level::Write && !write_supported(service) {
                continue;
            }
            if level_scopes(service, level)
                .iter()
                .all(|scope| has_scope(granted, scope))
            {
                selection.insert(service, level);
            }
        }
    }
    selection
}

/// Whether `scope` is granted, directly or through its [`broader_scope`] (logins leave
/// `scope` out when they request the broader one).
///
/// # Example
///
/// ```ignore
/// assert!(has_scope(&[url("calendar")], "calendar.readonly"));
/// assert!(has_scope(&["https://mail.google.com/".into()], "gmail.readonly"));
/// assert!(!has_scope(&[url("calendar.readonly")], "calendar"));
/// ```
fn has_scope(granted: &[String], scope: &str) -> bool {
    let expected = scope_url(&scope);
    let broader = broader_scope(&expected);
    granted
        .iter()
        .any(|granted| granted == &expected || Some(granted.as_str()) == broader)
}

/// Plain-language labels for every service row, in UI order.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::scopes;
/// assert_eq!(scopes::catalogue().len(), 15);
/// ```
pub fn catalogue() -> Vec<ServiceInfo> {
    catalogue_for(&Selection::new())
}

/// Builds the catalogue using the scopes already granted to the connected account.
///
/// Administrative rows are enabled only when their read scope is already present.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::{scopes, Level, Selection, Service};
/// let granted = Selection::from_levels([(Service::AdminReports, Level::Read)]);
/// let catalogue = scopes::catalogue_for(&granted);
/// assert!(catalogue.iter().find(|info| info.service == Service::AdminReports).unwrap().enabled);
/// ```
pub fn catalogue_for(granted: &Selection) -> Vec<ServiceInfo> {
    Service::ALL
        .into_iter()
        .map(|service| {
            let (name, read, write) = match service {
                Service::Gmail => ("Gmail", "Leer correos", "Acceso completo al correo"),
                Service::Calendar => (
                    "Calendar",
                    "Ver calendarios y eventos",
                    "Acceso completo a calendarios y eventos",
                ),
                Service::Chat => (
                    "Chat",
                    "Leer espacios, mensajes, miembros, reacciones y estado",
                    "Acceso completo a espacios, mensajes y miembros",
                ),
                Service::Meet => (
                    "Meet",
                    "Ver reuniones y sus transcripciones",
                    "Crear espacios de reunión",
                ),
                Service::Drive => (
                    "Drive",
                    "Ver y descargar archivos",
                    "Acceso completo a archivos",
                ),
                Service::Docs => ("Docs", "Leer documentos", "Acceso completo a documentos"),
                Service::Sheets => (
                    "Sheets",
                    "Leer hojas de cálculo",
                    "Acceso completo a hojas de cálculo",
                ),
                Service::Slides => (
                    "Slides",
                    "Leer presentaciones",
                    "Acceso completo a presentaciones",
                ),
                Service::Forms => (
                    "Forms",
                    "Leer formularios y respuestas",
                    "Acceso completo a formularios",
                ),
                Service::Tasks => ("Tasks", "Leer listas de tareas", "Acceso completo a tareas"),
                Service::People => (
                    "People",
                    "Leer contactos, perfiles y directorio",
                    "Acceso completo a contactos",
                ),
                Service::AdminReports => (
                    "Informes de administración",
                    "Leer informes de auditoría y uso",
                    "No disponible para esta API",
                ),
                Service::AdminDirectory => (
                    "Directorio de administración",
                    "Leer usuarios del directorio",
                    "Acceso completo a usuarios del directorio",
                ),
                Service::AdminGroups => (
                    "Grupos de administración",
                    "Leer grupos y membresías",
                    "Acceso completo a grupos y membresías",
                ),
                Service::CloudIdentity => (
                    "Cloud Identity",
                    "Leer grupos de identidad de la organización",
                    "Acceso completo a grupos de identidad",
                ),
            };
            ServiceInfo {
                service,
                name,
                read,
                write,
                category: if matches!(
                    service,
                    Service::AdminReports
                        | Service::AdminDirectory
                        | Service::AdminGroups
                        | Service::CloudIdentity
                ) {
                    ServiceCategory::Admin
                } else {
                    ServiceCategory::Workspace
                },
                write_enabled: write_supported(service),
                enabled: !matches!(
                    service,
                    Service::AdminReports
                        | Service::AdminGroups
                        | Service::CloudIdentity
                ) || granted.contains(service, Level::Read),
                availability_reason: if matches!(
                    service,
                    Service::AdminReports
                        | Service::AdminDirectory
                        | Service::AdminGroups
                        | Service::CloudIdentity
                ) && !granted.contains(service, Level::Read) {
                    Some("Requiere una cuenta de administrador de Google Workspace o permisos de administrador delegado.")
                } else {
                    None
                },
            }
        })
        .collect()
}

/// Business APIs that use the shared Google account but need dedicated connectors.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::scopes;
/// assert_eq!(scopes::business_catalogue().len(), 6);
/// ```
pub fn business_catalogue() -> Vec<BusinessConnectorInfo> {
    business_catalogue_for(&BusinessSelection::new())
}

/// Builds the business connector catalogue with currently granted access.
pub fn business_catalogue_for(granted: &BusinessSelection) -> Vec<BusinessConnectorInfo> {
    [
        (
            BusinessConnector::Youtube,
            "YouTube",
            "Leer canales, videos, comentarios y metadatos de analítica",
            "Subir y administrar videos",
        ),
        (
            BusinessConnector::GoogleAds,
            "Google Ads",
            "Leer campañas, anuncios, métricas y cuentas",
            "Crear y administrar campañas",
        ),
        (
            BusinessConnector::Analytics,
            "Google Analytics",
            "Leer propiedades, informes y métricas",
            "Administrar la configuración de propiedades seleccionadas",
        ),
        (
            BusinessConnector::SearchConsole,
            "Search Console",
            "Leer sitios, consultas y datos de rendimiento",
            "Administrar verificaciones y configuración de sitios",
        ),
        (
            BusinessConnector::BusinessProfile,
            "Business Profile",
            "Leer ubicaciones, reseñas y rendimiento",
            "Administrar ubicaciones y respuestas",
        ),
        (
            BusinessConnector::MerchantCenter,
            "Merchant Center",
            "Leer productos, cuentas y diagnósticos",
            "Administrar productos y feeds",
        ),
    ]
    .into_iter()
    .map(|(connector, name, read, write)| BusinessConnectorInfo {
        connector,
        name,
        read,
        write,
        enabled: connector == BusinessConnector::Youtube
            || granted.contains(connector, Level::Read),
        availability_reason: None,
    })
    .collect()
}
