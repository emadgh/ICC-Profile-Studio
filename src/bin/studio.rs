#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use gpui::{prelude::*, *};
use color_convert_rust::{Profile, backend::{self,Job,Mode}, inspection::{self,Inspection}};
use std::{path::PathBuf,sync::mpsc,time::Duration};

#[derive(Clone,Copy,PartialEq,Eq)]
enum Page { Dashboard, Inspector, Validator, Rebuilder, Comparison, Curves, Gamut, Correction, Converter, Lab, Settings, About }
const PAGES: [(Page,&str);12] = [
 (Page::Dashboard,"Dashboard"),(Page::Inspector,"Profile Inspector"),(Page::Validator,"Profile Validator"),(Page::Rebuilder,"Profile Rebuilder"),
 (Page::Comparison,"Profile Comparison"),(Page::Curves,"Tone Curves"),(Page::Gamut,"Gamut Viewer"),(Page::Correction,"Color Correction"),(Page::Converter,"Image Converter"),(Page::Lab,"Profiling Lab"),(Page::Settings,"Settings"),(Page::About,"About & Updates")
];
enum Event { Progress(String), Done(Result<String,String>) }
struct Studio {
 page:Page, source:Option<PathBuf>, profile:Option<Profile>, report:Option<Inspection>, error:Option<String>,
 caps:[f64;4],tac:f64,grid:usize,mode:Mode,status:String,busy:bool,
}
impl Default for Studio {
 fn default()->Self{Self{page:Page::Dashboard,source:None,profile:None,report:None,error:None,caps:[100.,100.,100.,0.],tac:240.,grid:33,mode:Mode::Custom,status:"Open an ICC or ICM profile to start.".into(),busy:false}}
}
fn button(id: impl Into<ElementId>,label:impl Into<SharedString>)->Stateful<Div>{
 div().id(id).px_3().py_2().rounded_md().bg(rgb(0x263449)).text_color(rgb(0xe6eefb)).cursor_pointer().hover(|s|s.bg(rgb(0x354963))).child(label.into())
}
fn heading(title:&str,sub:&str)->Div{div().flex().flex_col().gap_1().child(div().text_2xl().child(title.to_owned())).child(div().text_sm().text_color(rgb(0x94a3b8)).child(sub.to_owned()))}
impl Studio {
 fn open(&mut self) {
  if self.busy{return}
  let Some(path)=rfd::FileDialog::new().add_filter("ICC profile",&["icc","icm"]).pick_file() else{return};
  self.source=Some(path.clone());self.report=None;self.profile=None;self.error=None;
  match inspection::inspect(&path){Ok(r)=>self.report=Some(r),Err(e)=>self.error=Some(e)}
  if let Ok(p)=Profile::read(&path){
   if p.check().is_ok(){
    self.caps=if p.cmyk(){[100.,100.,100.,0.]}else{[100.;4]};
    self.tac=p.estimated_tac_percent().unwrap_or(240.);
    self.mode=if p.version_major()==2&&p.cmyk(){Mode::Argyll}else{Mode::Custom};
    self.grid=if p.version_major()==4{33}else{17};
    self.profile=Some(p);
   }
  }
  self.status=if self.report.is_some(){"Profile loaded with LittleCMS.".into()}else{"Profile could not be opened.".into()};
  self.page=Page::Inspector;
 }
 fn generate(&mut self,cx:&mut Context<Self>){
  if self.busy||self.profile.is_none(){return}
  let Some(source)=self.source.clone()else{return};
  let default_name=format!("{}_limited.icc",source.file_stem().unwrap_or_default().to_string_lossy());
  let Some(dest)=rfd::FileDialog::new().add_filter("ICC profile",&["icc"]).set_file_name(default_name).save_file()else{return};
  let mode=if self.caps[..3].iter().all(|&v|v==100.){self.mode}else{Mode::Custom};
  let job=Job{source,destination:dest,mode,caps:self.caps,tac:self.tac,estimate:false,grid:self.grid};
  let (tx,rx)=mpsc::channel();
  self.busy=true;self.status="Starting rebuild…".into();cx.notify();
  std::thread::spawn(move||{
   let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||backend::run(job,|s|{let _=tx.send(Event::Progress(s));}))).unwrap_or_else(|_|Err("Rebuild panicked".into()));
   let _=tx.send(Event::Done(result));
  });
  cx.spawn(async move |this,cx|{
   loop {
    let mut finished=false;
    match rx.try_recv(){
     Ok(Event::Progress(s))=>{if this.update(cx,|view,cx|{view.status=s;cx.notify();}).is_err(){break}}
     Ok(Event::Done(result))=>{let _=this.update(cx,|view,cx|{view.status=result.unwrap_or_else(|e|format!("Error: {e}"));view.busy=false;cx.notify();});finished=true;}
     Err(mpsc::TryRecvError::Disconnected)=>{let _=this.update(cx,|view,cx|{view.status="Worker disconnected".into();view.busy=false;cx.notify();});finished=true;}
     Err(mpsc::TryRecvError::Empty)=>{}
    }
    if finished{break}
    cx.background_executor().timer(Duration::from_millis(70)).await;
   }
  }).detach();
 }
 fn inspector(&self)->Div{
  let mut panel=div().flex().flex_col().gap_3().child(heading("Profile Inspector","ICC metadata decoded by LittleCMS 2."));
  if let Some(e)=&self.error{panel=panel.child(div().text_color(rgb(0xfca5a5)).child(e.clone()));}
  if let Some(r)=&self.report{
   panel=panel.child(format!("ICC v{:.2}  |  {}  |  {}",r.version,r.color_space,r.pcs))
    .child(format!("Class: {}  |  {:.2} MiB",r.device_class,r.size as f64/1048576.))
    .child(format!("Estimated total ink: {}",r.tac.map(|v|format!("{v:.1}%")).unwrap_or("Not available".into())))
    .child(div().text_lg().child(format!("Tags ({})",r.tags.len())))
    .children(r.tags.iter().map(|tag|div().px_2().py_1().rounded_md().bg(rgb(0x1b2637)).child(tag.clone())));
  } else {panel=panel.child("Select a reference profile first.");}
  panel
 }
 fn validator(&self)->Div{
  let mut panel=div().flex().flex_col().gap_3().child(heading("Profile Validator","LittleCMS transform smoke checks; not proof of print accuracy."));
  if let Some(e)=&self.error{panel=panel.child(format!("Open failed: {e}"));}
  if let Some(r)=&self.report{
   panel=panel.child("ICC opened by LittleCMS.");
   if r.intents.is_empty(){panel=panel.child("CMYK-specific checks are not available for this color space.");}
   for check in &r.intents{
    let line=match &check.result{Ok(max)=>format!("Transforms created • sampled maxima {:.2} / {:.2} / {:.2} / {:.2}%",max[0],max[1],max[2],max[3]),Err(e)=>format!("Failed: {e}")};
    panel=panel.child(div().flex().flex_col().gap_1().p_3().rounded_md().bg(rgb(0x1b2637)).child(check.name).child(div().text_sm().child(line)));
   }
   panel=panel.child("729 sRGB samples per intent. Strict output LUT bounds are checked separately by the rebuild engine.");
  }
  panel
 }
 fn rebuilder(&self,cx:&mut Context<Self>)->Div{
  let mut panel=div().flex().flex_col().gap_3().child(heading("Profile Rebuilder","Limit individual inks without changing the channel count."));
  let Some(profile)=&self.profile else{return panel.child("Open a supported four-channel printer profile.")};
  let names=profile.channels();
  for i in 0..4 {
   let name=names.get(i).cloned().unwrap_or(format!("Channel {}",i+1));
   panel=panel.child(div().flex().items_center().gap_2().p_2().rounded_md().bg(rgb(0x1b2637))
    .child(div().w(px(120.)).child(name))
    .child(button(("minus",i),"−").on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.caps[i]=(this.caps[i]-5.).max(0.);cx.notify();}})))
    .child(div().w(px(60.)).child(format!("{:.0}%",self.caps[i])))
    .child(button(("plus",i),"+").on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.caps[i]=(this.caps[i]+5.).min(100.);cx.notify();}})))
    .child(button(("off",i),"0%").on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.caps[i]=0.;cx.notify();}})))
    .child(button(("full",i),"100%").on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.caps[i]=100.;cx.notify();}}))));
  }
  panel=panel.child(div().flex().gap_2().items_center().child("K presets:").children([0,5,10].into_iter().map(|v|button(("k-preset",v as usize),format!("{v}%")).on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.caps[3]=v as f64;cx.notify();}})))))
   .child(div().flex().items_center().gap_2().child(format!("TAC: {:.0}%",self.tac))
    .child(button("tac-down","−5").on_click(cx.listener(|this,_,_,cx|{if !this.busy{this.tac=(this.tac-5.).max(0.);cx.notify();}})))
    .child(button("tac-up","+5").on_click(cx.listener(|this,_,_,cx|{if !this.busy{this.tac=(this.tac+5.).min(400.);cx.notify();}}))))
   .child(div().flex().gap_2().items_center().child(format!("Grid: {}³",self.grid)).children([9,17,25,33].into_iter().map(|v|button(("grid",v),v.to_string()).on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.grid=v;cx.notify();}})))))
   .child(format!("Engine: {}",if self.mode==Mode::Argyll&&self.caps[..3].iter().all(|&v|v==100.){"ArgyllCMS revfix"}else{"Custom constrained LUT"}))
   .child(button("generate",if self.busy{"Working…"}else{"Generate and verify ICC"}).on_click(cx.listener(|this,_,_,cx|this.generate(cx))))
   .child("Confirm the physical ink order and safe TAC. Validate the printed result after firing.");
  panel
 }
}
impl Render for Studio {
 fn render(&mut self,_window:&mut Window,cx:&mut Context<Self>)->impl IntoElement{
  let sidebar=div().flex().flex_col().w(px(224.)).flex_shrink_0().h_full().p_3().gap_1().bg(rgb(0x101a2a))
   .child(div().text_lg().mb_3().child("ICC Profile Studio"))
   .child(button("open-profile","Open ICC / ICM").on_click(cx.listener(|this,_,_,cx|{this.open();cx.notify();})))
   .children(PAGES.into_iter().enumerate().map(|(i,(page,label))|{
    let active=self.page==page;
    div().id(("page",i)).px_3().py_2().rounded_md().text_sm().cursor_pointer()
     .bg(rgb(if active{0x304867}else{0x101a2a})).hover(|s|s.bg(rgb(0x233751))).child(label)
     .on_click(cx.listener(move|this,_,_,cx|{this.page=page;cx.notify();}))
   }));
  let content=match self.page{
   Page::Dashboard=>div().flex().flex_col().gap_3().child(heading("Dashboard","Color management workspace for ceramic printing.")).child("Open an ICC profile to inspect its structure or rebuild a separation."),
   Page::Inspector=>self.inspector(),
   Page::Validator=>self.validator(),
   Page::Rebuilder=>self.rebuilder(cx),
   Page::Settings=>div().flex().flex_col().gap_3().child(heading("Settings","Engine configuration")).child("LittleCMS is linked statically. ArgyllCMS is installed by the existing portable backend when required."),
   Page::About=>div().flex().flex_col().gap_3().child(heading("ICC Profile Studio","Experimental GPUI frontend")).child(format!("Core version {}",env!("CARGO_PKG_VERSION"))).child("Developed by emadgh. LittleCMS 2 · ArgyllCMS · Rust.").child("Updater integration is pending in this frontend. Use the stable executable for updates."),
   other=>div().flex().flex_col().gap_3().child(heading(PAGES.iter().find(|(p,_)|*p==other).map(|(_,n)|*n).unwrap_or("Tool"),"Planned module")).child("Not implemented yet. See ROADMAP.md."),
  };
  div().flex().size_full().bg(rgb(0x152031)).text_color(rgb(0xe2e8f0)).child(sidebar)
   .child(div().flex().flex_col().flex_1().min_w(px(0.)).h_full()
    .child(div().px_5().py_3().bg(rgb(0x1b2637)).child(self.source.as_ref().map(|p|p.display().to_string()).unwrap_or("No profile selected".into())))
    .child(div().id("content-scroll").flex_1().overflow_y_scroll().p_5().child(content))
    .child(div().px_5().py_3().bg(rgb(0x101a2a)).text_sm().child(self.status.clone())))
 }
}
fn main(){
 Application::new().run(|cx:&mut App|{
  let bounds=Bounds::centered(None,size(px(1120.),px(820.)),cx);
  cx.open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(bounds)),..Default::default()},|_,cx|cx.new(|_|Studio::default())).expect("Open Studio window");
  cx.activate(true);
 });
}
