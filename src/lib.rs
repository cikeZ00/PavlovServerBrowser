mod known_versions;

use gloo_net::http::Request;
use gloo_timers::callback::Interval;
use js_sys::Date;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlSelectElement, Storage};
use yew::prelude::*;

use known_versions::KNOWN_VERSIONS;

const SERVER_LIST_URL: &str =
    "https://prod2-crossplay-pavlov-ms.vankrupt.net/servers/v2/list/{}/steam/0/0/0/all";
const DEFAULT_VERSION: &str = "1.0.27";

const STORAGE_KEY_AUTO_REFRESH: &str = "psb.auto_refresh";
const STORAGE_KEY_SORT_BY: &str = "psb.sort_by";
const STORAGE_KEY_REFRESH_INTERVAL: &str = "psb.refresh_interval";
const STORAGE_KEY_SELECTED_VERSION: &str = "psb.selected_version";
const STORAGE_KEY_CUSTOM_VERSIONS: &str = "psb.custom_versions";

fn browser_storage() -> Option<Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn storage_get(key: &str) -> Option<String> {
    browser_storage()?.get_item(key).ok().flatten()
}

fn storage_set(key: &str, value: &str) {
    if let Some(storage) = browser_storage() {
        let _ = storage.set_item(key, value);
    }
}

fn load_bool(key: &str, default: bool) -> bool {
    storage_get(key)
        .and_then(|raw| match raw.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

fn load_u32(key: &str, default: u32, min_value: u32) -> u32 {
    storage_get(key)
        .and_then(|raw| raw.parse::<u32>().ok())
        .filter(|value| *value >= min_value)
        .unwrap_or(default)
}

fn load_custom_versions() -> Vec<String> {
    storage_get(STORAGE_KEY_CUSTOM_VERSIONS)
        .map(|raw| {
            raw.lines()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn serialize_custom_versions(versions: &[String]) -> String {
    versions.join("\n")
}

fn is_known_version(version: &str) -> bool {
    KNOWN_VERSIONS
        .iter()
        .any(|known| known.eq_ignore_ascii_case(version))
}

fn merge_version_options(custom_versions: &[String]) -> Vec<String> {
    let mut options = KNOWN_VERSIONS
        .iter()
        .map(|version| (*version).to_string())
        .collect::<Vec<_>>();

    for custom_version in custom_versions {
        if !options
            .iter()
            .any(|known| known.eq_ignore_ascii_case(custom_version))
        {
            options.push(custom_version.clone());
        }
    }

    options
}

fn format_server_updated_timestamp(raw: &str) -> String {
    let date = Date::new(&JsValue::from_str(raw));

    if date.get_time().is_nan() {
        raw.to_string()
    } else {
        date.to_string().into()
    }
}

async fn fetch_server_list(version: &str) -> Option<ServerList> {
    let url = SERVER_LIST_URL.replace("{}", version);
    let response = Request::get(&url).send().await.ok()?;
    response.json::<ServerList>().await.ok()
}

#[derive(Deserialize, Debug, Clone)]
struct Server {
    name: String,
    slots: u32,
    #[serde(rename = "maxSlots")]
    max_slots: u32,
    #[serde(rename = "mapId")]
    map_id: String,
    #[serde(rename = "mapLabel")]
    map_label: String,
    port: u16,
    #[serde(rename = "bPasswordProtected")]
    b_password_protected: bool,
    #[serde(rename = "bSecured")]
    b_secured: bool,
    #[serde(rename = "gameMode")]
    game_mode: String,
    #[serde(rename = "gameModeLabel")]
    game_mode_label: String,
    ip: String,
    version: String,
    updated: String,
}

#[derive(Deserialize, Debug, Clone)]
struct ServerList {
    servers: Vec<Server>,
}

#[derive(Clone, PartialEq)]
enum SortCriteria {
    Name,
    Slots,
}

impl SortCriteria {
    fn from_storage_value(value: &str) -> Self {
        match value {
            "Name" => SortCriteria::Name,
            "Slots" => SortCriteria::Slots,
            _ => SortCriteria::Slots,
        }
    }

    fn as_storage_value(&self) -> &'static str {
        match self {
            SortCriteria::Name => "Name",
            SortCriteria::Slots => "Slots",
        }
    }
}

#[function_component(App)]
fn app() -> Html {
    let servers = use_state(|| Vec::<Server>::new());
    let search_query = use_state(|| "".to_string());
    let refresh_interval = use_state(|| load_u32(STORAGE_KEY_REFRESH_INTERVAL, 60, 5));
    let auto_refresh = use_state(|| load_bool(STORAGE_KEY_AUTO_REFRESH, false));
    let custom_versions = use_state(load_custom_versions);
    let version = use_state(|| {
        storage_get(STORAGE_KEY_SELECTED_VERSION).unwrap_or_else(|| DEFAULT_VERSION.to_string())
    });
    let version_to_add = use_state(String::new);
    let sort_criteria = use_state(|| {
        storage_get(STORAGE_KEY_SORT_BY)
            .map(|value| SortCriteria::from_storage_value(&value))
            .unwrap_or(SortCriteria::Slots)
    });

    let is_version_popout_open = use_state(|| false);

    let version_options = merge_version_options(&custom_versions);
    let selected_version = (*version).clone();
    let can_remove_selected_version = custom_versions
        .iter()
        .any(|saved| saved.eq_ignore_ascii_case(&selected_version));

    {
        let version = version.clone();
        let version_options = version_options.clone();
        use_effect_with_deps(
            move |deps: &(String, Vec<String>)| -> Box<dyn FnOnce()> {
                let (selected, options) = deps;

                if let Some(matched) = options
                    .iter()
                    .find(|option| option.eq_ignore_ascii_case(selected))
                {
                    if matched != selected {
                        version.set(matched.clone());
                        storage_set(STORAGE_KEY_SELECTED_VERSION, matched);
                    }
                } else if let Some(first) = options.first() {
                    version.set(first.clone());
                    storage_set(STORAGE_KEY_SELECTED_VERSION, first);
                }

                Box::new(|| {})
            },
            (selected_version.clone(), version_options.clone()),
        );
    }

    // Callback to fetch server data using the specified version.
    let fetch_data = {
        let servers = servers.clone();
        let version = version.clone();
        Callback::from(move |_| {
            let servers = servers.clone();
            let version = (*version).trim().to_string();
            if version.is_empty() {
                return;
            }

            spawn_local(async move {
                if let Some(server_list) = fetch_server_list(&version).await {
                    servers.set(server_list.servers);
                }
            });
        })
    };

    {
        let refresh_interval_val = *refresh_interval;
        let auto_refresh_val = *auto_refresh;
        let version_val = (*version).clone();
        let fetch_data = fetch_data.clone();
        use_effect_with_deps(
            move |deps: &(u32, bool, String)| -> Box<dyn FnOnce()> {
                let (interval, auto, _version) = deps;
                fetch_data.emit(());
                if *auto {
                    let timer = Interval::new(*interval * 1000, move || {
                        fetch_data.emit(());
                    });
                    Box::new(move || drop(timer))
                } else {
                    Box::new(|| {})
                }
            },
            (refresh_interval_val, auto_refresh_val, version_val),
        );
    }

    // Filter and sort
    let mut filtered_servers: Vec<Server> = servers
        .iter()
        .cloned()
        .filter(|server| {
            let query = search_query.to_lowercase();
            if query.is_empty() {
                true
            } else {
                server.name.to_lowercase().contains(&query)
                    || server.map_label.to_lowercase().contains(&query)
            }
        })
        .collect();

    match *sort_criteria {
        SortCriteria::Name => {
            filtered_servers.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }
        SortCriteria::Slots => {
            filtered_servers.sort_by(|a, b| b.slots.cmp(&a.slots));
        }
    }

    let on_toggle_version_popout = {
        let is_version_popout_open = is_version_popout_open.clone();
        Callback::from(move |_e: MouseEvent| {
            let current = *is_version_popout_open;
            is_version_popout_open.set(!current);
        })
    };

    // Handler for search input.
    let on_search = {
        let search_query = search_query.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            search_query.set(input.value());
        })
    };

    // Handler for refresh interval change.
    let on_interval_change = {
        let refresh_interval = refresh_interval.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Ok(val) = input.value().parse::<u32>() {
                if val >= 5 {
                    storage_set(STORAGE_KEY_REFRESH_INTERVAL, &val.to_string());
                    refresh_interval.set(val);
                }
            }
        })
    };

    // Handler for toggling auto refresh.
    let on_toggle_auto = {
        let auto_refresh = auto_refresh.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let checked = input.checked();
            storage_set(STORAGE_KEY_AUTO_REFRESH, if checked { "true" } else { "false" });
            auto_refresh.set(checked);
        })
    };

    // Handler for version selector.
    let on_version_change = {
        let version = version.clone();
        Callback::from(move |e: Event| {
            let input: HtmlSelectElement = e.target_unchecked_into();
            let selected = input.value();
            storage_set(STORAGE_KEY_SELECTED_VERSION, &selected);
            version.set(selected);
        })
    };

    let on_new_version_input = {
        let version_to_add = version_to_add.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            version_to_add.set(input.value());
        })
    };

    let on_add_version = {
        let custom_versions = custom_versions.clone();
        let version = version.clone();
        let version_to_add = version_to_add.clone();
        Callback::from(move |_e: MouseEvent| {
            let candidate = (*version_to_add).trim().to_string();
            if candidate.is_empty() {
                return;
            }

            if !is_known_version(&candidate) {
                let mut updated = (*custom_versions).clone();
                if !updated
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&candidate))
                {
                    updated.push(candidate.clone());
                    storage_set(
                        STORAGE_KEY_CUSTOM_VERSIONS,
                        &serialize_custom_versions(&updated),
                    );
                    custom_versions.set(updated);
                }
            }

            storage_set(STORAGE_KEY_SELECTED_VERSION, &candidate);
            version.set(candidate);
            version_to_add.set(String::new());
        })
    };

    let on_remove_version = {
        let custom_versions = custom_versions.clone();
        let version = version.clone();
        Callback::from(move |_e: MouseEvent| {
            let selected = (*version).clone();
            if is_known_version(&selected) {
                return;
            }

            let mut updated = (*custom_versions).clone();
            let previous_len = updated.len();
            updated.retain(|existing| !existing.eq_ignore_ascii_case(&selected));
            if updated.len() == previous_len {
                return;
            }

            let options = merge_version_options(&updated);
            let next_version = options
                .first()
                .cloned()
                .unwrap_or_else(|| DEFAULT_VERSION.to_string());

            storage_set(
                STORAGE_KEY_CUSTOM_VERSIONS,
                &serialize_custom_versions(&updated),
            );
            storage_set(STORAGE_KEY_SELECTED_VERSION, &next_version);

            custom_versions.set(updated);
            version.set(next_version);
        })
    };

    // Handler for sort criteria change.
    let on_sort_change = {
        let sort_criteria = sort_criteria.clone();
        Callback::from(move |e: Event| {
            let input: HtmlSelectElement = e.target_unchecked_into();
            let value = input.value();
            let criteria = SortCriteria::from_storage_value(&value);
            storage_set(STORAGE_KEY_SORT_BY, criteria.as_storage_value());
            sort_criteria.set(criteria);
        })
    };

    let sort_value = match *sort_criteria {
        SortCriteria::Name => "Name",
        SortCriteria::Slots => "Slots",
    };

    html! {
    <div class="app">
        <header class="top-app-bar">
            <div class="top-app-bar__inner">
                <div class="top-app-bar__title">{ "Pavlov Server Browser" }</div>
            </div>
        </header>

        <main class="content">
            <div class="controls-container">
                <section class="surface-card">
                    <div class="m3-section-title">{ "Search & Sort" }</div>
                    <div class="controls__grid">
                        <label class="m3-field">
                            <span class="m3-label">{ "Search" }</span>
                            <input
                                id="search"
                                class="m3-input"
                                type="search"
                                value={(*search_query).clone()}
                                oninput={on_search}
                                placeholder="Server or map name"
                            />
                        </label>

                        <label class="m3-field">
                            <span class="m3-label">{ "Sort by" }</span>
                            <select id="sort" class="m3-select" value={sort_value} onchange={on_sort_change}>
                                <option value="Name">{ "Name" }</option>
                                <option value="Slots">{ "Slots" }</option>
                            </select>
                        </label>
                    </div>
                </section>

                <section class="surface-card">
                    <div class="m3-section-title">{ "Settings" }</div>
                    <div class="controls__grid">
                        <div class="version-manager">
                            <div class="version-input-group">
                                <label class="m3-field" style="flex: 1;">
                                    <span class="m3-label">{ "Version" }</span>
                                    <select id="version" class="m3-select" value={(*version).clone()} onchange={on_version_change}>
                                        { for version_options.iter().map(|option| html! {
                                            <option value={option.clone()}>{ option.clone() }</option>
                                        })}
                                    </select>
                                    <span class="m3-helper">{ format!("{} repo versions, {} custom", KNOWN_VERSIONS.len(), custom_versions.len()) }</span>
                                </label>
                                <button
                                    type="button"
                                    class={classes!("m3-icon-button", if *is_version_popout_open { "sub-active" } else { "" })}
                                    onclick={on_toggle_version_popout}
                                    title="Manage versions"
                                >
                                    { "+" }
                                </button>
                            </div>

                            if *is_version_popout_open {
                                <div class="version-popout">
                                    <div class="version-actions">
                                        <input
                                            id="new_version"
                                            class="m3-input"
                                            type="text"
                                            value={(*version_to_add).clone()}
                                            oninput={on_new_version_input}
                                            placeholder="Add version (e.g. 1.0.28)"
                                        />
                                        <button type="button" class="m3-button" onclick={on_add_version}>{ "Add" }</button>
                                        <button
                                            type="button"
                                            class="m3-button m3-button--danger"
                                            onclick={on_remove_version}
                                            disabled={!can_remove_selected_version}
                                        >
                                            { "Remove selected" }
                                        </button>
                                    </div>
                                </div>
                            }
                        </div>

                        <label class="m3-field">
                            <span class="m3-label">{ "Refresh interval (sec)" }</span>
                            <input
                                id="interval"
                                class="m3-input"
                                type="number"
                                min="5"
                                value={refresh_interval.to_string()}
                                oninput={on_interval_change}
                            />
                        </label>

                        <div class="m3-field">
                            <span class="m3-label" style="visibility: hidden;">{ "\u{00A0}" }</span>
                            <label class="m3-checkbox">
                                <input
                                    id="auto_refresh"
                                    class="m3-checkbox__input"
                                    type="checkbox"
                                    checked={*auto_refresh}
                                    onchange={on_toggle_auto}
                                />
                                <span class="m3-checkbox__label">{ "Auto refresh" }</span>
                            </label>
                        </div>
                    </div>
                </section>
            </div>

            {
                if filtered_servers.is_empty() {
                    html! { <div class="empty-state">{ "No servers match your search." }</div> }
                } else {
                    html! {
                        <div class="mdc-data-table">
                            <table>
                                <thead>
                                    <tr>
                                        <th>{ "Server Name" }</th>
                                        <th>{ "Map" }</th>
                                        <th>{ "Game Mode" }</th>
                                        <th class="cell-numeric">{ "Slots" }</th>
                                        <th>{ "Version" }</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    { for filtered_servers.iter().map(|server| html! {
                                        <tr>
                                            <td data-label="Server Name">
                                                <div class="td-content">
                                                    <div class="server-label">{ &server.name }</div>
                                                    <div class="server-sub-label">{ format!("{}:{}", server.ip, server.port) }</div>
                                                </div>
                                            </td>
                                            <td data-label="Map">
                                                <div class="td-content">
                                                    <div class="server-label">{ &server.map_label }</div>
                                                    <div class="server-sub-label">{ &server.map_id }</div>
                                                </div>
                                            </td>
                                            <td data-label="Game Mode">
                                                <div class="td-content">
                                                    <div class="server-label">{ &server.game_mode_label }</div>
                                                    <div class="server-sub-label">{ &server.game_mode }</div>
                                                </div>
                                            </td>
                                            <td class="cell-numeric" data-label="Slots">
                                                <div class="td-content">
                                                    <div class="server-label">{ format!("{}/{}", server.slots, server.max_slots) }</div>
                                                </div>
                                            </td>
                                            <td data-label="Version">
                                                <div class="td-content">
                                                    <div class="server-label">{ &server.version }</div>
                                                    <div class="server-sub-label">{ format_server_updated_timestamp(&server.updated) }</div>
                                                </div>
                                            </td>
                                        </tr>
                                    })}
                                </tbody>
                            </table>
                        </div>
                    }
                }
            }
        </main>
    </div>
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    yew::Renderer::<App>::new().render();
}
