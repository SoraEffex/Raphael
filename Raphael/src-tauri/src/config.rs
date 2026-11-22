use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process::Command};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind { AppLauncher, OpenUrl, ShellCommand, HttpWebhook }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: ToolKind,
    pub target: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "yes")]
    pub require_approval: bool,
    #[serde(default)]
    pub builtin: bool,
}
fn yes() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub port: u16,
    pub token: String,
    pub accent: String,
    pub require_approval: bool,
    pub tools: Vec<ToolDef>,
}
impl Default for AppConfig {
    fn default() -> Self {
        Self { port: 8080, token: new_token(), accent: "#F97316".into(), require_approval: true, tools: vec![
            ToolDef { id:"open-calculator".into(), name:"Open Calculator".into(), description:"Open the system calculator application".into(), kind:ToolKind::AppLauncher, target:"calculator".into(), enabled:true, require_approval:false, builtin:true },
            ToolDef { id:"open-google".into(), name:"Open Google".into(), description:"Open Google in the default browser".into(), kind:ToolKind::OpenUrl, target:"https://www.google.com".into(), enabled:true, require_approval:false, builtin:true },
            ToolDef { id:"system-status".into(), name:"System Status".into(), description:"Return basic Raphael server status".into(), kind:ToolKind::ShellCommand, target:"status".into(), enabled:true, require_approval:false, builtin:true }
        ] }
    }
}
fn new_token() -> String {
    use rand::RngCore;
    let mut b=[0u8;32]; rand::thread_rng().fill_bytes(&mut b); hex::encode(b)
}
fn config_path(app: &tauri::AppHandle) -> Result<PathBuf> {
    let dir=app.path().app_config_dir()?; fs::create_dir_all(&dir)?; Ok(dir.join("config.yaml"))
}
pub fn load(app: &tauri::AppHandle) -> Result<AppConfig> {
    let p=config_path(app)?;
    if !p.exists() { let c=AppConfig::default(); save_at(&p,&c)?; return Ok(c); }
    let raw=fs::read_to_string(&p).context("read config.yaml")?;
    let mut c:AppConfig=serde_yaml::from_str(&raw).context("parse config.yaml")?;
    if c.token.is_empty() { c.token=new_token(); save_at(&p,&c)?; }
    Ok(c)
}
fn save_at(p:&PathBuf,c:&AppConfig)->Result<()> { fs::write(p,serde_yaml::to_string(c)?)?; Ok(()) }
pub fn save(app:&tauri::AppHandle,c:&AppConfig)->Result<()> { save_at(&config_path(app)?,c) }

pub async fn run_tool(app:&tauri::AppHandle, tool:&ToolDef)->Result<String> {
    match tool.kind {
        ToolKind::OpenUrl => {
            let u=url::Url::parse(&tool.target)?;
            if !matches!(u.scheme(),"http"|"https") { anyhow::bail!("Only http/https URLs are allowed"); }
            app.opener().open_url(tool.target.clone(),None::<&str>)?;
            Ok(format!("Opened {}",tool.target))
        },
        ToolKind::AppLauncher => {
            let name=tool.target.to_lowercase();
            #[cfg(target_os="windows")]
            let (program,args):(&str,Vec<&str>)=match name.as_str(){"calculator"=>("calc",vec![]),"notepad"=>("notepad",vec![]),_=>anyhow::bail!("App is not in the safe launcher allowlist")};
            #[cfg(target_os="macos")]
            let (program,args):(&str,Vec<&str>)=match name.as_str(){"calculator"=>("open",vec!["-a","Calculator"]),"textedit"=>("open",vec!["-a","TextEdit"]),_=>anyhow::bail!("App is not in the safe launcher allowlist")};
            #[cfg(target_os="linux")]
            let (program,args):(&str,Vec<&str>)=match name.as_str(){"calculator"=>("gnome-calculator",vec![]),"text-editor"=>("gedit",vec![]),_=>anyhow::bail!("App is not in the safe launcher allowlist")};
            Command::new(program).args(args).spawn().context("launch application")?;
            Ok(format!("Launched {}",name))
        },
        ToolKind::ShellCommand => {
            if tool.target=="status" { return Ok("Raphael is running. Use the Home tab to inspect the MCP server.".into()); }
            anyhow::bail!("Arbitrary shell commands are disabled in this build. Use the safe app launcher or webhook tool.")
        },
        ToolKind::HttpWebhook => {
            let u=url::Url::parse(&tool.target)?;
            if !matches!(u.scheme(),"http"|"https") { anyhow::bail!("Webhook must use http/https"); }
            let response=reqwest::Client::new().post(u).timeout(std::time::Duration::from_secs(10)).send().await?;
            Ok(format!("Webhook returned HTTP {}",response.status()))
        }
    }
}
