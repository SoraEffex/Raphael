#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main(){if std::env::args().any(|a|a=="--stdio"){raphael_lib::run_stdio();}else{raphael_lib::run();}}
