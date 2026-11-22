use serde_json::{json,Value};
use std::io::{self,BufRead,Write};

// MCP stdio transport: one JSON-RPC message per line; stdout is reserved for protocol data.
pub fn run(){
    let stdin=io::stdin();let mut out=io::stdout();
    for line in stdin.lock().lines(){
        let Ok(line)=line else {break}; if line.trim().is_empty(){continue;}
        let parsed:Result<Value,_>=serde_json::from_str(&line);let Ok(req)=parsed else {continue};
        let id=req.get("id").cloned().unwrap_or(Value::Null);
        let method=req.get("method").and_then(Value::as_str).unwrap_or("");
        let params=req.get("params").cloned().unwrap_or(json!({}));
        let result=match method{
            "initialize"=>Some(json!({"protocolVersion":"2025-03-26","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"raphael-stdio","version":"0.1.0"}})),
            "notifications/initialized"=>None,
            "ping"=>Some(json!({})),
            "tools/list"=>Some(json!({"tools":[
                {"name":"open-calculator","description":"Open the system calculator","inputSchema":{"type":"object","properties":{},"additionalProperties":false}},
                {"name":"open-google","description":"Open Google in the default browser","inputSchema":{"type":"object","properties":{},"additionalProperties":false}},
                {"name":"system-status","description":"Return Raphael status","inputSchema":{"type":"object","properties":{},"additionalProperties":false}}
            ]})),
            "tools/call"=>{
                let name=params.get("name").and_then(Value::as_str).unwrap_or("");
                let result=match name{
                    "system-status"=>Ok("Raphael stdio transport is running".to_string()),
                    "open-google"=>open_url("https://www.google.com"),
                    "open-calculator"=>open_calculator(),
                    _=>Err("Unknown tool".to_string())
                };
                Some(match result{Ok(s)=>json!({"content":[{"type":"text","text":s}]}),Err(e)=>json!({"isError":true,"content":[{"type":"text","text":e}]})})
            },
            _=>{let msg=json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method not found"}});let _=writeln!(out,"{}",msg);let _=out.flush();continue;}
        };
        if let Some(result)=result{let msg=json!({"jsonrpc":"2.0","id":id,"result":result});let _=writeln!(out,"{}",msg);let _=out.flush();}
    }
}
fn open_url(url:&str)->Result<String,String>{
    let parsed=url::Url::parse(url).map_err(|e|e.to_string())?;if !matches!(parsed.scheme(),"https"|"http"){return Err("Only http/https URLs are allowed".into());}
    #[cfg(target_os="windows")] let mut c={let mut x=std::process::Command::new("cmd");x.args(["/C","start","",url]);x};
    #[cfg(target_os="macos")] let mut c={let mut x=std::process::Command::new("open");x.arg(url);x};
    #[cfg(target_os="linux")] let mut c={let mut x=std::process::Command::new("xdg-open");x.arg(url);x};
    c.spawn().map_err(|e|e.to_string())?;Ok(format!("Opened {url}"))
}
fn open_calculator()->Result<String,String>{
    #[cfg(target_os="windows")] let mut c=std::process::Command::new("calc");
    #[cfg(target_os="macos")] let mut c={let mut x=std::process::Command::new("open");x.args(["-a","Calculator"]);x};
    #[cfg(target_os="linux")] let mut c=std::process::Command::new("gnome-calculator");
    c.spawn().map_err(|e|e.to_string())?;Ok("Calculator launched".into())
}
