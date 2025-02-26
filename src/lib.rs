use wasm_bindgen::prelude::*;
use yew::prelude::*;
use serde::Deserialize;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use gloo_timers::callback::Interval;

#[derive(Deserialize, Debug, Clone)]
struct Server {
    name: String,
    slots: u32,
    maxSlots: u32,
    mapId: String,
    mapLabel: String,
    port: u16,
    bPasswordProtected: bool,
    bSecured: bool,
    gameMode: String,
    gameModeLabel: String,
    ip: String,
    version: String,
    updated: String,
}

#[derive(Deserialize, Debug, Clone)]
struct ServerList {
    servers: Vec<Server>,
}

#[function_component(App)]
fn app() -> Html {
    let servers = use_state(|| Vec::<Server>::new());
    let search_query = use_state(|| "".to_string());
    let refresh_interval = use_state(|| 60u32); // in seconds
    let auto_refresh = use_state(|| false);
    let version = use_state(|| "1.0.24".to_string());

    // Callback to fetch server data using the specified version.
    let fetch_data = {
        let servers = servers.clone();
        let version = version.clone();
        Callback::from(move |_| {
            let servers = servers.clone();
            let version = (*version).clone();
            spawn_local(async move {
                let url = format!(
                    "https://prod-crossplay-pavlov-ms.vankrupt.net/servers/v2/list/{}/steam/0/0/0/all",
                    version
                );
                if let Ok(resp) = Request::get(&url).send().await {
                    if let Ok(server_list) = resp.json::<ServerList>().await {
                        servers.set(server_list.servers);
                    }
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
    // Filter
    let mut filtered_servers: Vec<Server> = servers
        .iter()
        .cloned()
        .filter(|server| {
            let query = search_query.to_lowercase();
            if query.is_empty() {
                true
            } else {
                server.name.to_lowercase().contains(&query)
                    || server.mapLabel.to_lowercase().contains(&query)
            }
        })
        .collect();
    filtered_servers.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    // Handler for search input.
    let on_search = {
        let search_query = search_query.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            search_query.set(input.value());
        })
    };

    // Handler for refresh interval change.
    let on_interval_change = {
        let refresh_interval = refresh_interval.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            if let Ok(val) = input.value().parse::<u32>() {
                refresh_interval.set(val);
            }
        })
    };

    // Handler for toggling auto refresh.
    let on_toggle_auto = {
        let auto_refresh = auto_refresh.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            auto_refresh.set(input.checked());
        })
    };

    // Handler for version selector.
    let on_version_change = {
        let version = version.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            version.set(input.value());
        })
    };

    html! {
        <div class="container" style="padding-top: 20px;">
            <h1 class="center-align" style="color: #9c27b0;">{ "Pavlov Server Browser" }</h1>
            <div class="row">
                <div class="input-field col s12 m3">
                    <input id="search" type="text" value={(*search_query).clone()} oninput={on_search} />
                    <label for="search" class="active" style="color: #9c27b0;">{ "Search" }</label>
                </div>
                <div class="input-field col s12 m3">
                    <input id="interval" type="number" min="5" value={refresh_interval.to_string()} oninput={on_interval_change} />
                    <label for="interval" class="active" style="color: #9c27b0;">{ "Refresh Interval (sec)" }</label>
                </div>
                <div class="input-field col s12 m3">
                    <input id="version" type="text" value={(*version).clone()} oninput={on_version_change} />
                    <label for="version" class="active" style="color: #9c27b0;">{ "Version" }</label>
                </div>
                <div class="input-field col s12 m3">
                    <p>
                      <label style="color: #9c27b0;">
                        <input id="auto_refresh" type="checkbox" checked={*auto_refresh} onchange={on_toggle_auto} />
                        <span>{ "Auto Refresh" }</span>
                      </label>
                    </p>
                </div>
            </div>
            <div class="row card-container">
                { for filtered_servers.iter().map(|server| html! {
                    <div class="col s12 m6 l4">
                        <div class="card hoverable">
                            <div class="card-content">
                                <span class="card-title">{ &server.name }</span>
                                <p>{ format!("IP: {}", server.ip) }</p>
                                <p>{ format!("Map: {} (ID: {})", server.mapLabel, server.mapId) }</p>
                                <p>{ format!("Game Mode: {} (Label: {})", server.gameMode, server.gameModeLabel) }</p>
                                <p>{ format!("Version: {}", server.version) }</p>
                                <p>{ format!("Updated: {}", server.updated) }</p>
                                <p>{ format!("Slots: {}/{}", server.slots, server.maxSlots) }</p>
                            </div>
                        </div>
                    </div>
                })}
            </div>
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    yew::Renderer::<App>::new().render();
}
