use fraccional_googleworkspace_mcp_core::{BusinessConnector, Core, Level, Service, Status};
use tauri::{
    AppHandle,
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
};

#[derive(Clone)]
pub(crate) enum TrayState {
    Connecting,
    Status(Status),
    Error,
}

pub(crate) fn refresh_tray_menu(app: &AppHandle, core: &Core) {
    let state = match core.status() {
        Ok(status) => TrayState::Status(status),
        Err(error) => {
            eprintln!("Could not refresh tray status: {error}");
            TrayState::Error
        }
    };
    set_tray_menu(app, state);
}

pub(crate) fn set_tray_menu(app: &AppHandle, state: TrayState) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };

    let connecting = matches!(state, TrayState::Connecting);
    let icon_bytes: &[u8] = if connecting {
        &include_bytes!("../icons/tray-icon-busy.png")[..]
    } else {
        &include_bytes!("../icons/tray-icon.png")[..]
    };
    match Image::from_bytes(icon_bytes) {
        Ok(icon) => {
            if let Err(error) = tray.set_icon(Some(icon)) {
                eprintln!("Could not update tray icon: {error}");
                return;
            }
            if let Err(error) = tray.set_icon_as_template(!connecting) {
                eprintln!("Could not update tray icon template mode: {error}");
                return;
            }
        }
        Err(error) => {
            eprintln!("Could not load tray icon: {error}");
            return;
        }
    }

    let presentation = presentation(&state);
    let status_item = match MenuItemBuilder::with_id("status", presentation.status_text)
        .enabled(false)
        .build(app)
    {
        Ok(item) => item,
        Err(error) => {
            eprintln!("Could not build tray status item: {error}");
            return;
        }
    };
    let account_item =
        match MenuItemBuilder::with_id(presentation.account_action, presentation.account_text)
            .build(app)
        {
            Ok(item) => item,
            Err(error) => {
                eprintln!("Could not build tray account item: {error}");
                return;
            }
        };
    let open = match MenuItemBuilder::with_id("open", "Abrir configuración").build(app) {
        Ok(item) => item,
        Err(error) => {
            eprintln!("Could not build tray open item: {error}");
            return;
        }
    };
    let skills = match MenuItemBuilder::with_id("skills", "Abrir skills").build(app) {
        Ok(item) => item,
        Err(error) => {
            eprintln!("Could not build tray skills item: {error}");
            return;
        }
    };
    let connect_agent =
        match MenuItemBuilder::with_id("connect-agent", "Conectar un agente…").build(app) {
            Ok(item) => item,
            Err(error) => {
                eprintln!("Could not build tray connect agent item: {error}");
                return;
            }
        };
    let quit = match MenuItemBuilder::with_id("quit", "Salir de fraccional-googleworkspace-mcp")
        .build(app)
    {
        Ok(item) => item,
        Err(error) => {
            eprintln!("Could not build tray quit item: {error}");
            return;
        }
    };
    let permission_items: Vec<_> = presentation
        .permissions
        .into_iter()
        .enumerate()
        .filter_map(|(index, label)| {
            MenuItemBuilder::with_id(format!("permission-{index}"), label)
                .enabled(false)
                .build(app)
                .map_err(|error| {
                    eprintln!("Could not build tray permission item: {error}");
                    error
                })
                .ok()
        })
        .collect();

    let mut builder = MenuBuilder::new(app)
        .item(&status_item)
        .item(&account_item)
        .separator();
    for item in &permission_items {
        builder = builder.item(item);
    }
    let menu = match builder
        .item(&open)
        .separator()
        .item(&connect_agent)
        .item(&skills)
        .separator()
        .item(&quit)
        .build()
    {
        Ok(menu) => menu,
        Err(error) => {
            eprintln!("Could not build tray menu: {error}");
            return;
        }
    };
    if let Err(error) = tray.set_menu(Some(menu)) {
        eprintln!("Could not update tray menu: {error}");
    }
}

struct TrayPresentation {
    status_text: String,
    account_action: &'static str,
    account_text: String,
    permissions: Vec<String>,
}

fn presentation(state: &TrayState) -> TrayPresentation {
    match state {
        TrayState::Connecting => TrayPresentation {
            status_text: "🟢 Operación en curso…".to_string(),
            account_action: "open",
            account_text: "Abrir fraccional-googleworkspace-mcp".to_string(),
            permissions: Vec::new(),
        },
        TrayState::Error => TrayPresentation {
            status_text: "🔴 Estado no disponible".to_string(),
            account_action: "open",
            account_text: "Abrir fraccional-googleworkspace-mcp".to_string(),
            permissions: Vec::new(),
        },
        TrayState::Status(status) => {
            let (indicator, account_action, account_text) = match status.account.as_deref() {
                Some(account) if status.token_valid => {
                    ("🟢 Conectado", "open", account.to_string())
                }
                Some(account) => ("🟡 Reautenticación requerida", "open", account.to_string()),
                None => (
                    "⚪ Sin conexión",
                    "connect",
                    "Conectar cuenta de Google".to_string(),
                ),
            };
            TrayPresentation {
                status_text: indicator.to_string(),
                account_action,
                account_text,
                permissions: active_permission_labels(status),
            }
        }
    }
}

fn active_permission_labels(status: &Status) -> Vec<String> {
    let mut permissions = Vec::new();
    for service in Service::ALL {
        if status.granted.contains(service, Level::Write) {
            permissions.push(format!("{} — Acceso completo", service_label(service)));
        } else if status.granted.contains(service, Level::Read) {
            permissions.push(format!("{} — Solo lectura", service_label(service)));
        }
    }
    for connector in [
        BusinessConnector::Youtube,
        BusinessConnector::GoogleAds,
        BusinessConnector::Analytics,
        BusinessConnector::SearchConsole,
        BusinessConnector::BusinessProfile,
        BusinessConnector::MerchantCenter,
    ] {
        if status.business_granted.contains(connector, Level::Write) {
            permissions.push(format!("{} — Acceso completo", business_label(connector)));
        } else if status.business_granted.contains(connector, Level::Read) {
            permissions.push(format!("{} — Solo lectura", business_label(connector)));
        }
    }
    permissions
}

fn service_label(service: Service) -> &'static str {
    match service {
        Service::Gmail => "Gmail",
        Service::Calendar => "Calendar",
        Service::Chat => "Chat",
        Service::Meet => "Meet",
        Service::Drive => "Drive",
        Service::Docs => "Docs",
        Service::Sheets => "Sheets",
        Service::Slides => "Slides",
        Service::Forms => "Forms",
        Service::Tasks => "Tasks",
        Service::People => "People",
        Service::AdminReports => "Admin Reports",
        Service::AdminDirectory => "Admin Directory",
        Service::AdminGroups => "Admin Groups",
        Service::CloudIdentity => "Cloud Identity",
    }
}

fn business_label(connector: BusinessConnector) -> &'static str {
    match connector {
        BusinessConnector::Youtube => "YouTube",
        BusinessConnector::GoogleAds => "Google Ads",
        BusinessConnector::Analytics => "Analytics",
        BusinessConnector::SearchConsole => "Search Console",
        BusinessConnector::BusinessProfile => "Business Profile",
        BusinessConnector::MerchantCenter => "Merchant Center",
    }
}

#[cfg(test)]
mod tests {
    use fraccional_googleworkspace_mcp_core::{BusinessSelection, KeyStorage, Selection, Status};

    use super::{TrayState, presentation};

    fn status(account: Option<&str>, token_valid: bool) -> Status {
        Status {
            gws_available: true,
            account: account.map(str::to_owned),
            granted: Selection::new(),
            business_granted: BusinessSelection::new(),
            token_valid,
            config_dir: "/tmp/gws".into(),
            config_dir_is_default: true,
            key_storage: KeyStorage::Auto,
            resolved_key_storage: KeyStorage::Keyring,
            foreign_credentials: false,
        }
    }

    #[test]
    fn disconnected_account_item_connects() {
        let menu = presentation(&TrayState::Status(status(None, false)));

        assert_eq!(menu.status_text, "⚪ Sin conexión");
        assert_eq!(menu.account_action, "connect");
        assert_eq!(menu.account_text, "Conectar cuenta de Google");
    }

    #[test]
    fn valid_account_item_opens_settings() {
        let menu = presentation(&TrayState::Status(status(Some("ana@example.com"), true)));

        assert_eq!(menu.status_text, "🟢 Conectado");
        assert_eq!(menu.account_action, "open");
        assert_eq!(menu.account_text, "ana@example.com");
    }
}
