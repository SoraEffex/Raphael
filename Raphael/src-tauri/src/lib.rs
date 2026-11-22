mod config;
mod mcp_server;
mod stdio;
use config::{AppConfig,ToolDef};
use std::{collections::HashMap,sync::Arc};
use tokio::sync::{Mutex,oneshot};
use tauri::{menu::{Menu,MenuItem},tray::TrayIconBuilder,Manager,WindowEvent};
use tauri_plugin_opener::OpenerExt;

pub struct AppState {
    config:Arc<Mutex<AppConfig>>,
    server:Arc<Mutex<mcp_server::ServerControl>>,
    pending:Arc<Mutex<HashMap<String,oneshot::Sender<bool>>>>,
}
impl AppState { fn new(c:AppConfig)->Self { Self{config:Arc::new(Mutex::new(c)),server:Arc::new(Mutex::new(Default::default())),pending:Arc::new(Mutex::new(HashMap::new()))} } }

#[tauri::command]
async fn get_config(state:tauri::State<'_,AppState>)->Result<AppConfig,String>{Ok(state.config.lock().await.clone())}
#[tauri::command]
async fn save_config(app:tauri::AppHandle,state:tauri::State<'_,AppState>,config:AppConfig)->Result<(),String>{
    if !(1024..=65535).contains(&config.port){return Err("Port must be between 1024 and 65535".into());}
    if !config.accent.starts_with('#')||config.accent.len()!=7{return Err("Accent must be a hex color such as #F97316".into());}
    config::save(&app,&config).map_err(|e|e.to_string())?;*state.config.lock().await=config;Ok(())
}
#[tauri::command]
async fn start_server(app:tauri::AppHandle,state:tauri::State<'_,AppState>)->Result<String,String>{
    if state.server.lock().await.task.is_some(){return Ok("already_running".into());}
    let c=state.config.lock().await.clone();mcp_server::start(app,c.port,c.token).await.map_err(|e|e.to_string())?;Ok("started".into())
}
#[tauri::command]
async fn stop_server(app:tauri::AppHandle)->Result<(),String>{mcp_server::stop(&app).await;Ok(())}
#[tauri::command]
async fn server_status(state:tauri::State<'_,AppState>)->Result<bool,String>{Ok(state.server.lock().await.task.is_some())}
#[tauri::command]
async fn upsert_tool(app:tauri::AppHandle,state:tauri::State<'_,AppState>,tool:ToolDef)->Result<(),String>{
    if tool.id.trim().is_empty()||tool.name.trim().is_empty()||tool.description.trim().is_empty()||tool.target.trim().is_empty(){return Err("Please fill in every field".into());}
    let mut c=state.config.lock().await;
    if let Some(existing)=c.tools.iter_mut().find(|t|t.id==tool.id){let builtin=existing.builtin;*existing=ToolDef{builtin,..tool};}else{c.tools.push(tool);}
    config::save(&app,&c).map_err(|e|e.to_string())?;Ok(())
}
#[tauri::command]
async fn delete_tool(app:tauri::AppHandle,state:tauri::State<'_,AppState>,id:String)->Result<(),String>{let mut c=state.config.lock().await;c.tools.retain(|t|t.id!=id||t.builtin);config::save(&app,&c).map_err(|e|e.to_string())?;Ok(())}
#[tauri::command]
async fn set_tool_enabled(app:tauri::AppHandle,state:tauri::State<'_,AppState>,id:String,enabled:bool)->Result<(),String>{let mut c=state.config.lock().await;if let Some(t)=c.tools.iter_mut().find(|t|t.id==id){t.enabled=enabled;}config::save(&app,&c).map_err(|e|e.to_string())?;Ok(())}
#[tauri::command]
async fn run_test(app:tauri::AppHandle,state:tauri::State<'_,AppState>,id:String)->Result<String,String>{let t={let c=state.config.lock().await;c.tools.iter().find(|t|t.id==id).cloned().ok_or("Tool not found")?};config::run_tool(&app,&t).await.map_err(|e|e.to_string())}
#[tauri::command]
async fn resolve_approval(state:tauri::State<'_,AppState>,id:String,allow:bool)->Result<(),String>{let tx=state.pending.lock().await.remove(&id).ok_or("Approval request expired")?;tx.send(allow).map_err(|_|"Request already expired".into())}
#[tauri::command]
async fn reveal_config(app:tauri::AppHandle)->Result<(),String>{let p=app.path().app_config_dir().map_err(|e|e.to_string())?;app.opener().open_path(p.to_string_lossy().to_string(),None::<&str>).map_err(|e|e.to_string())}

pub fn run_stdio(){ stdio::run(); }

pub fn run(){
    tauri::Builder::default().plugin(tauri_plugin_opener::init())
    .setup(|app|{
        let cfg=config::load(&app.handle())?;app.manage(AppState::new(cfg));
        let open=MenuItem::with_id(app,"open","Open Raphael",true,None::<&str>)?;
        let start=MenuItem::with_id(app,"start","Start Server",true,None::<&str>)?;
        let stop=MenuItem::with_id(app,"stop","Stop Server",true,None::<&str>)?;
        let quit=MenuItem::with_id(app,"quit","Quit Raphael",true,None::<&str>)?;
        let menu=Menu::with_items(app,&[&open,&start,&stop,&quit])?;
        TrayIconBuilder::new().menu(&menu).tooltip("Raphael AI Bridge").on_menu_event(|app,event|match event.id().as_ref(){
            "open"=>{if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.set_focus();}},
            "start"=>{let h=app.clone();tauri::async_runtime::spawn(async move{let s=h.state::<AppState>();let c=s.config.lock().await.clone();let _=mcp_server::start(h.clone(),c.port,c.token).await;});},
            "stop"=>{let h=app.clone();tauri::async_runtime::spawn(async move{mcp_server::stop(&h).await;});},
            "quit"=>app.exit(0),_=>{}
        }).build(app)?;
        if let Some(w)=app.get_webview_window("main"){let _=w.set_title("Raphael — AI Desktop Bridge");}
        Ok(())
    })
    .on_window_event(|window,event|{if let WindowEvent::CloseRequested{api,..}=event{api.prevent_close();let _=window.hide();}})
    .invoke_handler(tauri::generate_handler![get_config,save_config,start_server,stop_server,server_status,upsert_tool,delete_tool,set_tool_enabled,run_test,resolve_approval,reveal_config])
    .run(tauri::generate_context!()).expect("error while running Raphael");
}
