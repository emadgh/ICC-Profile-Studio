//! Windows release discovery is restricted to the Studio beta namespace.
use std::{ffi::c_void, ptr::{null,null_mut},sync::{Arc,Mutex}};
use update_via_github::{UpdateConfig,UpdateInfo,UpdateStatus,download_update,apply_update};
use windows_sys::Win32::Networking::WinHttp::*;
use color_convert_rust::studio_release;
const MAX:usize=4*1024*1024;
struct Handle(*mut c_void);
impl Drop for Handle { fn drop(&mut self){if !self.0.is_null(){unsafe{WinHttpCloseHandle(self.0);}}} }
fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(Some(0)).collect()}
fn page(n:usize)->Result<Vec<u8>,String>{
    unsafe {
        let agent=wide("ICC-Profile-Studio");
        let session=Handle(WinHttpOpen(agent.as_ptr(),WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,null(),null(),0));
        if session.0.is_null(){return Err("Cannot initialize update connection.".into())}
        WinHttpSetTimeouts(session.0,5000,5000,10000,10000);
        let host=wide("api.github.com");
        let conn=Handle(WinHttpConnect(session.0,host.as_ptr(),INTERNET_DEFAULT_HTTPS_PORT,0));
        if conn.0.is_null(){return Err("Cannot connect to GitHub.".into())}
        let verb=wide("GET");
        let path=wide(&format!("/repos/emadgh/color-convert-rust/releases?per_page=30&page={n}"));
        let req=Handle(WinHttpOpenRequest(conn.0,verb.as_ptr(),path.as_ptr(),null(),null(),null(),WINHTTP_FLAG_SECURE|WINHTTP_FLAG_REFRESH));
        if req.0.is_null(){return Err("Cannot create release request.".into())}
        if WinHttpSendRequest(req.0,null(),0,null(),0,0,0)==0||WinHttpReceiveResponse(req.0,null_mut())==0{return Err("GitHub release request failed.".into())}
        let mut status=0u32;let mut len=4u32;let mut idx=0u32;
        if WinHttpQueryHeaders(req.0,WINHTTP_QUERY_STATUS_CODE|WINHTTP_QUERY_FLAG_NUMBER,null(),&mut status as *mut u32 as *mut c_void,&mut len,&mut idx)==0{return Err("Cannot read GitHub response.".into())}
        if status==404{return Err("Studio releases inaccessible (private repository or missing release access). Open GitHub Releases while signed in.".into())}
        if status!=200{return Err(format!("GitHub release request returned HTTP {status}."))}
        let mut result=Vec::new();let mut chunk=[0u8;16384];
        loop {let mut got=0u32;if WinHttpReadData(req.0,chunk.as_mut_ptr() as *mut c_void,chunk.len() as u32,&mut got)==0{return Err("Release response read failed.".into())}
          if got==0{break}if result.len()+got as usize>MAX{return Err("Release metadata exceeds limit.".into())}
          result.extend_from_slice(&chunk[..got as usize]);
        }
        Ok(result)
    }
}
fn check(config:&UpdateConfig)->Result<Option<UpdateInfo>,String>{
    let mut all=Vec::<serde_json::Value>::new();
    for n in 1..=3{
        let body=page(n)?;
        let mut rows:Vec<serde_json::Value>=serde_json::from_slice(&body).map_err(|e|e.to_string())?;
        let done=rows.len()<30;all.append(&mut rows);if done{break}
    }
    let bytes=serde_json::to_vec(&all).map_err(|e|e.to_string())?;
    Ok(studio_release::select(&bytes,&config.current_version)?.map(|c|UpdateInfo{version:c.version,release_url:c.release_url,download_url:c.download_url,checksum_url:c.checksum_url,asset_digest:c.asset_digest}))
}
pub struct StudioUpdater{config:Arc<UpdateConfig>,state:Arc<Mutex<UpdateStatus>>,retry:Arc<Mutex<Option<UpdateInfo>>>}
impl StudioUpdater{
    pub fn new(config:UpdateConfig)->Self{Self{config:Arc::new(config),state:Arc::new(Mutex::new(UpdateStatus::Idle)),retry:Arc::new(Mutex::new(None))}}
    pub fn status(&self)->UpdateStatus{self.state.lock().unwrap().clone()}
    pub fn start_check(&self,_auto:bool)->bool{
        {let mut s=self.state.lock().unwrap();if matches!(*s,UpdateStatus::Checking|UpdateStatus::Downloading{..}|UpdateStatus::Ready(..)){return false}*s=UpdateStatus::Checking;*self.retry.lock().unwrap()=None;}
        let state=self.state.clone();let config=self.config.clone();
        std::thread::spawn(move||{let next=match check(&config){Ok(Some(info))=>UpdateStatus::Available(info),Ok(None)=>UpdateStatus::UpToDate,Err(e)=>UpdateStatus::Failed(e)};*state.lock().unwrap()=next;});
        true
    }
    pub fn can_retry_download(&self)->bool{
        let state=self.state.lock().unwrap();matches!(*state,UpdateStatus::Failed(_))&&self.retry.lock().unwrap().is_some()
    }
    pub fn start_download(&self)->bool{
        let info={let mut s=self.state.lock().unwrap();
            let info=match &*s{UpdateStatus::Available(info)=>info.clone(),UpdateStatus::Failed(_)=>{let Some(info)=self.retry.lock().unwrap().clone()else{return false};info},_=>return false};
            *self.retry.lock().unwrap()=Some(info.clone());
            *s=UpdateStatus::Downloading{info:info.clone(),downloaded:0,total:None};info
        };
        let state=self.state.clone();let config=self.config.clone();
        std::thread::spawn(move||{let p=state.clone();let i=info.clone();let result=download_update(&config,&info,move|downloaded,total|{*p.lock().unwrap()=UpdateStatus::Downloading{info:i.clone(),downloaded,total};});
          *state.lock().unwrap()=match result{Ok(path)=>UpdateStatus::Ready(info,path),Err(e)=>UpdateStatus::Failed(e)};
        });true
    }
    pub fn apply_ready(&self)->Result<bool,String>{let UpdateStatus::Ready(_,path)=self.status() else{return Ok(false)};apply_update(&self.config,&path)?;Ok(true)}
}
