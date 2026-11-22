use axum::{extract::State, http::{HeaderMap,StatusCode}, response::{IntoResponse,Response,sse::{Event,Sse}}, routing::{get,post}, Json, Router};
use serde_json::{json,Value};
use std::{net::SocketAddr,convert::Infallible};
use tokio::{sync::{Mutex,oneshot},task::JoinHandle};
use tauri::{AppHandle,Emitter,Manager};
use crate::{config::{self,ToolDef},AppState};

#[derive(Clone)] struct HttpState { app:AppHandle, token:String }
pub struct ServerControl { pub stop:Option<oneshot::Sender<()>>, pub task:Option<JoinHandle<()>> }
impl Default for ServerControl { fn default()->Self { Self{stop:None,task:None} } }
fn authorized(h:&HeaderMap,token:&str)->bool { h.get("authorization").and_then(|v|v.to_str().ok()).map(|v|v==format!("Bearer {}",token)).unwrap_or(false) }
async fn health()->Json<Value>{Json(json!({"name":"Raphael","status":"online"}))}
async fn mcp(State(s):State<HttpState>,headers:HeaderMap,Json(body):Json<Value>)->Response {
    if !authorized(&headers,&s.token) { return (StatusCode::UNAUTHORIZED,Json(json!({"error":"Unauthorized"}))).into_response(); }
    let id=body.get("id").cloned().unwrap_or(Value::Null);
    let method=body.get("method").and_then(Value::as_str).unwrap_or("");
    let params=body.get("params").cloned().unwrap_or(json!({}));
    let result=match method {
        "initialize"=>json!({"protocolVersion":"2025-03-26","capabilities":{"tools":{"listChanged":true}},"serverInfo":{"name":"raphael","version":"0.1.0"}}),
        "notifications/initialized"=>return StatusCode::ACCEPTED.into_response(),
        "ping"=>json!({}),
        "tools/list"=>{
            let state=s.app.state::<AppState>(); let cfg=state.config.lock().await;
            let tools:Vec<Value>=cfg.tools.iter().filter(|t|t.enabled).map(|t|json!({"name":t.id,"description":t.description,"inputSchema":{"type":"object","properties":{},"additionalProperties":false}})).collect();
            json!({"tools":tools})
        },
        "tools/call"=>{
            let name=params.get("name").and_then(Value::as_str).unwrap_or("");
            let state=s.app.state::<AppState>();
            let tool:Option<ToolDef>={let cfg=state.config.lock().await;cfg.tools.iter().find(|t|t.id==name&&t.enabled).cloned()};
            if let Some(tool)=tool {
                let approval= { let cfg=state.config.lock().await; cfg.require_approval && tool.require_approval };
                if approval {
                    let request_id=uuid_simple(); let (tx,rx)=oneshot::channel();
                    state.pending.lock().await.insert(request_id.clone(),tx);
                    let _=s.app.emit("approval-request",json!({"id":request_id,"name":tool.name,"description":tool.description,"target":tool.target}));
                    match tokio::time::timeout(std::time::Duration::from_secs(90),rx).await {
                        Ok(Ok(true))=>{}, _=>{state.pending.lock().await.remove(&request_id); return rpc_error(id,-32001,"Request denied or timed out").into_response();}
                    }
                }
                match config::run_tool(&s.app,&tool).await { Ok(msg)=>json!({"content":[{"type":"text","text":msg}]}),Err(e)=>json!({"isError":true,"content":[{"type":"text","text":e.to_string()}]}) }
            } else { return rpc_error(id,-32602,"Unknown or disabled tool").into_response(); }
        },
        _=>return rpc_error(id,-32601,"Method not found").into_response()
    };
    Json(json!({"jsonrpc":"2.0","id":id,"result":result})).into_response()
}
fn rpc_error(id:Value,code:i64,message:&str)->Json<Value>{Json(json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}}))}
fn uuid_simple()->String { use rand::RngCore;let mut b=[0u8;12];rand::thread_rng().fill_bytes(&mut b);hex::encode(b) }
async fn sse(State(s):State<HttpState>,headers:HeaderMap)->Response {
    if !authorized(&headers,&s.token) { return StatusCode::UNAUTHORIZED.into_response(); }
    let stream=async_stream::stream! {
        yield Ok::<Event,Infallible>(Event::default().event("endpoint").data("/messages"));
        loop { tokio::time::sleep(std::time::Duration::from_secs(15)).await; yield Ok(Event::default().comment("keep-alive")); }
    };
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default()).into_response()
}
async fn message(State(s):State<HttpState>,headers:HeaderMap,Json(body):Json<Value>)->Response { mcp(State(s),headers,Json(body)).await }
pub async fn start(app:AppHandle,port:u16,token:String)->anyhow::Result<()> {
    let state=HttpState{app:app.clone(),token};
    let router=Router::new().route("/health",get(health)).route("/mcp",post(mcp)).route("/sse",get(sse)).route("/messages",post(message)).with_state(state);
    let addr=SocketAddr::from(([127,0,0,1],port)); let listener=tokio::net::TcpListener::bind(addr).await?;
    let (tx,rx)=oneshot::channel();
    let task=tokio::spawn(async move { let _=axum::serve(listener,router).with_graceful_shutdown(async{let _=rx.await;}).await; });
    let control=app.state::<AppState>(); let mut server=control.server.lock().await; server.stop=Some(tx);server.task=Some(task);Ok(())
}
pub async fn stop(app:&AppHandle) { let control=app.state::<AppState>();let mut s=control.server.lock().await;if let Some(tx)=s.stop.take(){let _=tx.send(());}if let Some(t)=s.task.take(){let _=t.await;} }
