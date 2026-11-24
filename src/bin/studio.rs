#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use gpui::{prelude::*, *};
use color_convert_rust::{Profile, backend::{self,Job,Mode}, inspection::{self,Inspection}};
use std::{path::PathBuf,sync::mpsc,time::Duration};
use color_convert_rust::comparison::{self,Comparison};
use color_convert_rust::editing::{self,Edit};
#[path = "../gamut_view.rs"] mod gamut_view;
use color_convert_rust::gamut::{self,Overlay};
use gamut_view::ViewOptions;
use color_convert_rust::gradient::{self,GradientReport};

#[derive(Clone,Copy,PartialEq,Eq)]
enum Page { Dashboard, Inspector, Validator, Rebuilder, Comparison, Curves, Reorder, Metadata, Gamut, Gradient, Correction, Converter, Lab, Settings, About }
const PAGES: [(Page,&str);15] = [
 (Page::Dashboard,"Dashboard"),(Page::Inspector,"Profile Inspector"),(Page::Validator,"Profile Validator"),(Page::Rebuilder,"Profile Rebuilder"),
 (Page::Comparison,"Profile Comparison"),(Page::Curves,"Tone Curves"),(Page::Reorder,"Channel Reorder"),(Page::Metadata,"Profile Metadata"),(Page::Gamut,"Gamut Viewer"),(Page::Gradient,"Gradient Analyzer"),(Page::Correction,"Color Correction"),(Page::Converter,"Image Converter"),(Page::Lab,"Profiling Lab"),(Page::Settings,"Settings"),(Page::About,"About & Updates")
];
enum Event { Progress(String), Done(Result<String,String>) }
struct Studio {
 page:Page, source:Option<PathBuf>, profile:Option<Profile>, report:Option<Inspection>, error:Option<String>,
 caps:[f64;4],tac:f64,grid:usize,mode:Mode,status:String,busy:bool, candidate:Option<PathBuf>,comparison:Option<Comparison>,compare_error:Option<String>,compare_grid:usize,gamma:[f64;4],order:[usize;4], preview_open:bool, preview:Option<Overlay>, preview_options:ViewOptions, gamut_grid:usize,gradient:Option<GradientReport>,gradient_error:Option<String>,coverage_threshold:f64,
}
impl Default for Studio {
 fn default()->Self{Self{page:Page::Dashboard,source:None,profile:None,report:None,error:None,caps:[100.,100.,100.,0.],tac:240.,grid:33,mode:Mode::Custom,status:"Open an ICC or ICM profile to start.".into(),busy:false,candidate:None,comparison:None,compare_error:None,compare_grid:17,gamma:[1.;4],order:[0,1,2,3],preview_open:true,preview:None,preview_options:ViewOptions::default(),gamut_grid:17,gradient:None,gradient_error:None,coverage_threshold:2.}}
}
fn button(id: impl Into<ElementId>,label:impl Into<SharedString>)->Stateful<Div>{
 div().id(id).px_3().py_2().rounded_md().bg(rgb(0x263449)).text_color(rgb(0xe6eefb)).cursor_pointer().hover(|s|s.bg(rgb(0x354963))).child(label.into())
}
fn heading(title:&str,sub:&str)->Div{div().flex().flex_col().gap_1().child(div().text_2xl().child(title.to_owned())).child(div().text_sm().text_color(rgb(0x94a3b8)).child(sub.to_owned()))}
impl Studio {
 fn open(&mut self,cx:&mut Context<Self>) {
  if self.busy{return}
  let Some(path)=rfd::FileDialog::new().add_filter("ICC profile",&["icc","icm"]).pick_file() else{return};
  self.source=Some(path.clone());self.preview=None;self.candidate=None;self.gradient=None;self.gradient_error=None;self.gamma=[1.;4];self.order=[0,1,2,3];self.comparison=None;self.compare_error=None;self.report=None;self.profile=None;self.error=None;
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
  if self.preview_open && self.report.is_some(){self.refresh_gamut(cx);}
 }
 fn generate(&mut self,cx:&mut Context<Self>){
  if self.busy||self.profile.is_none(){return}
  let Some(source)=self.source.clone()else{return};
  let default_name=format!("{}_limited.icc",source.file_stem().unwrap_or_default().to_string_lossy());
  let Some(dest)=rfd::FileDialog::new().add_filter("ICC profile",&["icc"]).set_file_name(default_name).save_file()else{return};
  let mode=if self.caps[..3].iter().all(|&v|v==100.){self.mode}else{Mode::Custom};
  let output_path=dest.clone();
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
     Ok(Event::Done(result))=>{let _=this.update(cx,|view,cx|{view.busy=false;match result{Ok(s)=>{view.status=s;view.candidate=Some(output_path.clone());view.preview=None;view.comparison=None;view.gradient=None;view.gradient_error=None;if view.preview_open{view.refresh_gamut(cx);}},Err(e)=>view.status=format!("Error: {e}")}cx.notify();});finished=true;}
     Err(mpsc::TryRecvError::Disconnected)=>{let _=this.update(cx,|view,cx|{view.status="Worker disconnected".into();view.busy=false;cx.notify();});finished=true;}
     Err(mpsc::TryRecvError::Empty)=>{}
    }
    if finished{break}
    cx.background_executor().timer(Duration::from_millis(70)).await;
   }
  }).detach();
 }
 fn compare_profiles(&mut self,cx:&mut Context<Self>){
  if self.busy{return}
  let (Some(a),Some(b))=(self.source.clone(),self.candidate.clone())else{return};
  let grid=self.compare_grid;
  self.busy=true;self.comparison=None;self.compare_error=None;self.status="Comparing profile predictions with LittleCMS...".into();cx.notify();
  cx.spawn(async move |this,cx|{
   let result=cx.background_spawn(async move{comparison::compare(&a,&b,grid,lcms2::Intent::RelativeColorimetric)}).await;
   let _=this.update(cx,|view,cx|{
    match result{Ok(r)=>{view.status=format!("Compared {} sRGB samples and 33 neutrals.",r.count);view.comparison=Some(r)},Err(e)=>{view.status="Comparison failed".into();view.compare_error=Some(e)}}
    view.busy=false;cx.notify();
   });
  }).detach();
 }
 fn comparison_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Profile Comparison","Predicted D50 media-relative Lab appearance and CMYK separation."));
  p=p.child(format!("Reference: {}",self.source.as_ref().map(|p|p.display().to_string()).unwrap_or("Not selected".into())))
   .child(button("pick-candidate","Select candidate ICC").on_click(cx.listener(|this,_,_,cx|{
    if !this.busy{if let Some(path)=rfd::FileDialog::new().add_filter("ICC profile",&["icc","icm"]).pick_file(){this.candidate=Some(path);this.preview=None;this.comparison=None;this.gradient=None;this.gradient_error=None;this.compare_error=None;if this.preview_open{this.refresh_gamut(cx);}cx.notify();}}
   })))
   .child(format!("Candidate: {}",self.candidate.as_ref().map(|p|p.display().to_string()).unwrap_or("Not selected".into())))
   .child(div().flex().items_center().gap_2().child(format!("RGB sample grid: {}³",self.compare_grid)).children([9usize,17,33].into_iter().map(|v|button(("compare-grid",v),v.to_string()).on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.compare_grid=v;this.comparison=None;cx.notify();}})))))
   .child(button("compare-run",if self.busy{"Working…"}else{"Compare profiles"}).on_click(cx.listener(|this,_,_,cx|this.compare_profiles(cx))));
  if let Some(e)=&self.compare_error{p=p.child(div().text_color(rgb(0xfca5a5)).child(e.clone()));}
  if let Some(r)=&self.comparison{
   p=p.child(div().flex().gap_3().children([("Mean ΔE00",r.mean_de),("P95 ΔE00",r.p95_de),("Max ΔE00",r.max_de)].map(|(label,v)|div().flex_1().p_3().rounded_md().bg(rgb(0x1b2637)).child(div().text_sm().child(label)).child(div().text_2xl().child(format!("{v:.2}"))))))
    .child(format!("{} samples · worst source RGB {:?}",r.count,r.worst_rgb))
    .child(div().text_lg().child("Delta E distribution"));
   for (i,label) in ["< 1","1–2","2–3","3–5","≥ 5"].into_iter().enumerate(){
    let fraction=r.histogram[i] as f32/r.count as f32;
    p=p.child(div().flex().items_center().gap_2().child(div().w(px(55.)).child(label)).child(div().w(px(200.)).h(px(9.)).rounded_md().bg(rgb(0x283a51)).child(div().w(px(200.*fraction)).h_full().rounded_md().bg(rgb(0x60a5fa)))).child(format!("{:.1}%",100.*fraction)));
   }
   p=p.child(div().text_lg().child("Ink distribution (sampled)"));
   for j in 0..4{p=p.child(format!("{} — mean {:.2}% → {:.2}% | max {:.2}% → {:.2}%",["C","M","Y","K"][j],r.reference.mean[j],r.candidate.mean[j],r.reference.max[j],r.candidate.max[j]));}
   p=p.child(format!("Mean TAC {:.2}% → {:.2}%  |  Max sampled TAC {:.2}% → {:.2}%",r.reference.mean_tac,r.candidate.mean_tac,r.reference.max_tac,r.candidate.max_tac))
    .child(div().text_lg().child("Neutral axis (selected samples)"));
   for n in r.neutral.iter().step_by(4){p=p.child(format!("Gray {:3} | L* {:.2} → {:.2} | ΔE00 {:.2} | K {:.1}% → {:.1}%",n.rgb,n.reference_lab[0],n.candidate_lab[0],n.delta_e,n.reference_ink[3],n.candidate_ink[3]));}
   p=p.child("Relative Colorimetric, no black-point compensation. Predicted profile differences only; not measured print differences, gamut volume or production ink averages.");
  }
  p
 }
 fn save_edit(&mut self,kind:u8,cx:&mut Context<Self>){
  if self.busy{return}let Some(source)=self.source.clone()else{return};
  let suffix=match kind{0=>"curves",1=>"reordered",_=>"renamed"};
  let default_name=format!("{}_{}.icc",source.file_stem().unwrap_or_default().to_string_lossy(),suffix);
  let Some(mut destination)=rfd::FileDialog::new().add_filter("ICC profile",&["icc","icm"]).set_file_name(default_name).save_file()else{return};
  if destination.extension().is_none(){destination.set_extension("icc");}
  let output_path=destination.clone();
  let edit=match kind{0=>Edit::Tone(self.gamma),1=>Edit::Reorder(self.order),_=>Edit::Rename(destination.file_stem().unwrap_or_default().to_string_lossy().into_owned())};
  self.busy=true;self.status="Editing ICC and validating with LittleCMS…".into();cx.notify();
  cx.spawn(async move |this,cx|{
   let result=cx.background_spawn(async move{editing::edit_file(&source,&destination,edit)}).await;
   let _=this.update(cx,|view,cx|{view.busy=false;match result{Ok(s)=>{view.status=s;view.candidate=Some(output_path);view.preview=None;view.comparison=None;view.gradient=None;view.gradient_error=None;if view.preview_open{view.refresh_gamut(cx);}},Err(e)=>view.status=format!("Error: {e}")}cx.notify();});
  }).detach();
 }
 fn curves_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Tone Curves","Compose gamma corrections on output ink curves."));
  let Some(profile)=&self.profile else{return p.child("Open a supported printer profile first.")};
  for (i,name) in profile.channels().into_iter().enumerate(){
   let gamma=self.gamma[i];let color=[0x00b7e5,0xe33b91,0xe8c44d,0x9ca3af][i];
   p=p.child(div().p_3().rounded_md().bg(rgb(0x1b2637)).flex().flex_col().gap_2()
    .child(div().flex().items_center().gap_2().child(div().w(px(110.)).child(name))
    .child(button(("gamma-minus",i),"−").on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.gamma[i]=(this.gamma[i]-0.1).max(0.25);cx.notify();}})))
    .child(format!("γ {gamma:.2}")).child(button(("gamma-plus",i),"+").on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.gamma[i]=(this.gamma[i]+0.1).min(4.);cx.notify();}})))
    .child(button(("gamma-reset",i),"Reset").on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.gamma[i]=1.;cx.notify();}}))))
    .child(div().flex().gap_1().items_end().h(px(52.)).children((0..=20).map(|j|div().w(px(12.)).h(px(1.+50.*(j as f32/20.).powf(gamma as f32))).bg(rgb(color)))))
    .child(format!("25% → {:.1}%  |  50% → {:.1}%  |  75% → {:.1}%",25_f64.powf(gamma)/100_f64.powf(gamma-1.),100.*0.5_f64.powf(gamma),100.*0.75_f64.powf(gamma))));
  }
  p.child("Output curves change separations; A2B remains the original printer model. TAC and prior channel caps must be rechecked. This is not measured recalibration.")
   .child(button("save-curves","Save new ICC").on_click(cx.listener(|this,_,_,cx|this.save_edit(0,cx))))
 }
 fn reorder_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Channel Reorder","New channel index → original physical ink."));
  let Some(profile)=&self.profile else{return p.child("Open a supported four-channel profile first.")};
  let names=profile.channels();
  for i in 0..4{p=p.child(div().flex().items_center().gap_2().child(div().w(px(200.)).child(format!("{} → {}",i+1,names[self.order[i]])))
   .child(button(("order-up",i),"↑").on_click(cx.listener(move|this,_,_,cx|{if !this.busy&&i>0{this.order.swap(i,i-1);cx.notify();}})))
   .child(button(("order-down",i),"↓").on_click(cx.listener(move|this,_,_,cx|{if !this.busy&&i<3{this.order.swap(i,i+1);cx.notify();}}))));}
  p.child(button("reset-order","Reset mapping").on_click(cx.listener(|this,_,_,cx|{if !this.busy{this.order=[0,1,2,3];cx.notify();}})))
   .child("Changes both A2B input axes and B2A output channels. Unknown vendor tags or calibration data cause rejection. RIP channel order must match the new physical order.")
   .child(button("save-order","Save reordered ICC").on_click(cx.listener(|this,_,_,cx|this.save_edit(1,cx))))
 }
 fn metadata_page(&self,cx:&mut Context<Self>)->Div{
  div().flex().flex_col().gap_3().child(heading("Profile Metadata","Create a copy with a new internal description."))
   .child("The selected output filename (without extension) becomes the ICC description. All transform tags are retained. The original file is never overwritten.")
   .child(button("save-rename","Save renamed ICC").on_click(cx.listener(|this,_,_,cx|this.save_edit(2,cx))))
 }
 fn refresh_gamut(&mut self,cx:&mut Context<Self>){
  if self.busy{return}let Some(reference)=self.source.clone()else{return};
  let candidate=self.candidate.clone();let grid=self.gamut_grid;
  self.preview=None;self.busy=true;self.status="Sampling effective Lab gamut with LittleCMS…".into();cx.notify();
  cx.spawn(async move |this,cx|{
   let result=cx.background_spawn(async move{gamut::overlay(&reference,candidate.as_deref(),grid)}).await;
   let _=this.update(cx,|view,cx|{
    view.preview=Some(result);view.busy=false;view.status="Gamut analysis finished. See each profile's result in Preview.".into();cx.notify();
   });
  }).detach();
 }
 fn preview_panel(&self,cx:&mut Context<Self>)->Div{
  let mut panel=div().flex().flex_col().w(px(360.)).flex_shrink_0().h_full().bg(rgb(0x101a2a))
   .child(div().flex().items_center().justify_between().p_3().child(div().text_lg().child("Gamut Preview"))
    .child(button("collapse-preview","»").on_click(cx.listener(|this,_,_,cx|{this.preview_open=false;cx.notify();}))));
  panel=panel.child(div().id("preview-scroll").flex_1().overflow_y_scroll().p_3().child(self.gamut_controls(cx)));
  panel
 }
 fn gamut_controls(&self,cx:&mut Context<Self>)->Div{
  let o=self.preview_options;
  let mut p=div().flex().flex_col().gap_3()
   .child(div().flex().gap_2().child(button("view-2d",if o.three_d{"2D"}else{"2D ✓"}).on_click(cx.listener(|this,_,_,cx|{this.preview_options.three_d=false;cx.notify();})))
    .child(button("view-3d",if o.three_d{"3D ✓"}else{"3D"}).on_click(cx.listener(|this,_,_,cx|{this.preview_options.three_d=true;cx.notify();}))))
   .child(div().flex().gap_2().child(button("show-before",if o.reference{"Before ✓"}else{"Before"}).on_click(cx.listener(|this,_,_,cx|{this.preview_options.reference=!this.preview_options.reference;cx.notify();})))
    .child(button("show-after",if o.candidate{"After ✓"}else{"After"}).on_click(cx.listener(|this,_,_,cx|{this.preview_options.candidate=!this.preview_options.candidate;cx.notify();}))))
   .child(div().flex().gap_2().items_center().child(div().size(px(9.)).bg(rgb(0x60a5fa))).child("Reference").child(div().size(px(9.)).bg(rgb(0xfbbf24))).child("Candidate"))
   .child(div().text_xs().child(format!("Before: {}",self.source.as_ref().and_then(|p|p.file_name()).map(|n|n.to_string_lossy().into_owned()).unwrap_or("Not selected".into()))))
   .child(div().text_xs().child(format!("After: {}",self.candidate.as_ref().and_then(|p|p.file_name()).map(|n|n.to_string_lossy().into_owned()).unwrap_or("Not selected".into()))))
   .child(button("preview-pick","Select comparison ICC").on_click(cx.listener(|this,_,_,cx|{if !this.busy{if let Some(path)=rfd::FileDialog::new().add_filter("ICC profile",&["icc","icm"]).pick_file(){this.candidate=Some(path);this.preview=None;this.comparison=None;this.gradient=None;this.gradient_error=None;this.refresh_gamut(cx);cx.notify();}}})))
   .child(div().flex().items_center().gap_1().child("Grid").children([9usize,17,33].map(|v|button(("gamut-grid",v),if self.gamut_grid==v{format!("{v} ✓")}else{v.to_string()}).on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.gamut_grid=v;this.refresh_gamut(cx);cx.notify();}})))))
   .child(button("refresh-gamut",if self.busy{"Working…"}else{"Refresh gamut"}).on_click(cx.listener(|this,_,_,cx|this.refresh_gamut(cx))));
  if o.three_d{
   p=p.child(div().flex().gap_1().child(button("yaw-left","↶").on_click(cx.listener(|this,_,_,cx|{this.preview_options.yaw-=0.20;cx.notify();})))
    .child(button("yaw-right","↷").on_click(cx.listener(|this,_,_,cx|{this.preview_options.yaw+=0.20;cx.notify();})))
    .child(button("pitch-up","↑").on_click(cx.listener(|this,_,_,cx|{this.preview_options.pitch=(this.preview_options.pitch+0.15).min(1.55);cx.notify();})))
    .child(button("pitch-down","↓").on_click(cx.listener(|this,_,_,cx|{this.preview_options.pitch=(this.preview_options.pitch-0.15).max(-1.55);cx.notify();}))));
  }else{
   p=p.child(div().flex().gap_1().child(button("slice-toggle",if o.slice{"L* slice ✓"}else{"All L*"}).on_click(cx.listener(|this,_,_,cx|{this.preview_options.slice=!this.preview_options.slice;cx.notify();})))
    .child(button("slice-down","−").on_click(cx.listener(|this,_,_,cx|{this.preview_options.light=(this.preview_options.light-5.).max(0.);cx.notify();})))
    .child(format!("{:.0}",o.light))
    .child(button("slice-up","+").on_click(cx.listener(|this,_,_,cx|{this.preview_options.light=(this.preview_options.light+5.).min(100.);cx.notify();}))));
  }
  p=p.child(div().flex().gap_1().child(button("zoom-out","− zoom").on_click(cx.listener(|this,_,_,cx|{this.preview_options.zoom=(this.preview_options.zoom/1.2).max(0.5);cx.notify();})))
   .child(button("zoom-in","+ zoom").on_click(cx.listener(|this,_,_,cx|{this.preview_options.zoom=(this.preview_options.zoom*1.2).min(3.);cx.notify();})))
   .child(button("reset-view","Reset").on_click(cx.listener(|this,_,_,cx|{this.preview_options=ViewOptions::default();cx.notify();}))));
  if let Some(data)=&self.preview{
   match &data.reference{Ok(c)=>p=p.child(format!("Before: {} distinct / {} targets",c.points.len(),c.sampled)),Err(e)=>p=p.child(format!("Before failed: {e}"))}
   if let Some(c)=&data.candidate{match c{Ok(c)=>p=p.child(format!("After: {} distinct / {} targets",c.points.len(),c.sampled)),Err(e)=>p=p.child(format!("After failed: {e}"))}}
   p=p.child(button("mesh-toggle",if o.wireframe{"Wireframe ✓"}else{"Point cloud"}).on_click(cx.listener(|this,_,_,cx|{this.preview_options.wireframe=!this.preview_options.wireframe;cx.notify();})));
   p=p.child(gamut_view::plot(data,o));
   if o.wireframe{p=p.child("Convex hull wireframe (slice uses points).");}
   if o.three_d{p=p.child("Axes: red +a*, green +b*, white +L*");}else{p=p.child(if o.slice{format!("a* / b* | L* {:.0} ± 5",o.light)}else{"a* / b* | all lightness values".into()});}
  }else{p=p.child("Open an ICC or refresh to calculate the preview.");}
  p.child(div().text_xs().text_color(rgb(0x94a3b8)).child("Sampled effective separation gamut (Lab → B2A → device → A2B → Lab). Relative colorimetric, D50. Point cloud, not a measured or exact surface/volume."))
 }
 fn analyze_gradient(&mut self,cx:&mut Context<Self>){
  if self.busy{return}let Some(reference)=self.source.clone()else{return};
  let candidate=self.candidate.clone();self.gradient=None;self.gradient_error=None;self.busy=true;self.status="Analyzing 13 gradients with LittleCMS…".into();cx.notify();
  cx.spawn(async move |this,cx|{
   let result=cx.background_spawn(async move{gradient::compare(&reference,candidate.as_deref())}).await;
   let _=this.update(cx,|view,cx|{view.busy=false;match result{Ok(r)=>{view.gradient=Some(r);view.status="Gradient analysis complete.".into();},Err(e)=>{view.gradient_error=Some(e);view.status="Gradient analysis failed.".into();}}cx.notify();});
  }).detach();
 }
 fn gradient_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Gradient Analyzer","13 sRGB ramps × 256 samples, relative colorimetric."))
   .child(button("run-gradient","Analyze gradients").on_click(cx.listener(|this,_,_,cx|this.analyze_gradient(cx))))
   .child("Reference and optional candidate use the same input ramps. Plateau: ΔE00 < 0.05; spike: > 3× neighboring median + 0.5. Heuristics, not measured banding.");
  if let Some(e)=&self.gradient_error{p=p.child(div().text_color(rgb(0xfca5a5)).child(e.clone()));}
  if let Some(r)=&self.gradient{
   for (i,a) in r.reference.iter().enumerate(){
    let b=r.candidate.as_ref().and_then(|v|v.get(i));
    let scale=a.steps.iter().chain(b.into_iter().flat_map(|v|v.steps.iter())).copied().fold(0.1_f64,f64::max);
    let mut row=div().flex().flex_col().gap_1().p_3().rounded_md().bg(rgb(0x1b2637)).child(div().text_lg().child(a.name.clone()))
     .child(format!("Before: max ΔE {:.3} · P95 {:.3} · worst step {}→{} · plateaus {} · spikes {} · max ink step {:.2} pp",a.max_step,a.p95,a.worst_index,a.worst_index+1,a.plateaus,a.spikes,a.max_ink_step));
    if let Some(b)=b{row=row.child(format!("After: max ΔE {:.3} · P95 {:.3} · worst step {}→{} · plateaus {} · spikes {} · max ink step {:.2} pp",b.max_step,b.p95,b.worst_index,b.worst_index+1,b.plateaus,b.spikes,b.max_ink_step));}
    row=row.child(div().flex().items_end().h(px(52.)).children(a.steps.chunks(8).map(|v|div().flex_1().h(px((50.*v.iter().copied().fold(0.,f64::max)/scale) as f32+1.)).bg(rgb(0x60a5fa)))));
    if let Some(b)=b{row=row.child(div().flex().items_end().h(px(52.)).children(b.steps.chunks(8).map(|v|div().flex_1().h(px((50.*v.iter().copied().fold(0.,f64::max)/scale) as f32+1.)).bg(rgb(0xfbbf24)))));}
    p=p.child(row);
   }
  }
  p
 }
 fn gamut_analysis_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Gamut Viewer","Effective separation coverage and convex envelope."))
   .child(button("open-gamut-page-preview","Show preview pane").on_click(cx.listener(|this,_,_,cx|{this.preview_open=true;if this.preview.is_none(){this.refresh_gamut(cx);}cx.notify();})))
   .child(div().flex().gap_2().items_center().child("Target tolerance ΔE00").children([1usize,2,3,5].map(|v|button(("threshold",v),if self.coverage_threshold==v as f64{format!("{v} ✓")}else{v.to_string()}).on_click(cx.listener(move|this,_,_,cx|{this.coverage_threshold=v as f64;cx.notify();})))));
  if let Some(data)=&self.preview{
   if let (Ok(a),Some(Ok(b)))=(&data.reference,&data.candidate){
    if let Some(c)=gamut::coverage(a,b,self.coverage_threshold){
     p=p.child(format!("Matched target grid: {} valid D50 Lab targets",c.total));
     for (label,n,color) in [("Within tolerance in both",c.both,0x60a5fa),("Lost (before only)",c.lost,0xf87171),("Gained (after only)",c.gained,0x34d399),("Outside tolerance in both",c.neither,0x94a3b8)]{
      let f=n as f32/c.total as f32;p=p.child(div().flex().flex_col().gap_1().child(format!("{label}: {n} ({:.1}%)",100.*f)).child(div().h(px(9.)).bg(rgb(0x283a51)).child(div().w(relative(f)).h_full().bg(rgb(color)))));
     }
     let base=c.both+c.lost;
     if base>0{p=p.child(format!("Lost share of reference-reproduced targets: {:.2}%",100.*c.lost as f64/base as f64));}
    }
    if let (Some(a),Some(b))=(&a.hull,&b.hull){
     p=p.child(format!("Convex envelope volume (Lab³): {:.0} → {:.0} ({:+.2}%)",a.volume,b.volume,100.*(b.volume/a.volume-1.)));
    }
   }else{p=p.child("Select two valid CMYK printer profiles to calculate target coverage differences.");}
  }
  p.child("Coverage is a sampled round-trip reproduction test, not proof of physical achievability. Convex envelopes can include unattainable concave regions; volume is an approximation only.")
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
  let sidebar=div().id("sidebar").flex().flex_col().w(px(224.)).flex_shrink_0().h_full().overflow_y_scroll().p_3().gap_1().bg(rgb(0x101a2a))
   .child(div().text_lg().mb_3().child("ICC Profile Studio"))
   .child(button("open-profile","Open ICC / ICM").on_click(cx.listener(|this,_,_,cx|{this.open(cx);cx.notify();})))
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
   Page::Comparison=>self.comparison_page(cx),
   Page::Curves=>self.curves_page(cx),
   Page::Reorder=>self.reorder_page(cx),
   Page::Metadata=>self.metadata_page(cx),
   Page::Gamut=>self.gamut_analysis_page(cx),
   Page::Gradient=>self.gradient_page(cx),
   Page::Settings=>div().flex().flex_col().gap_3().child(heading("Settings","Engine configuration")).child("LittleCMS is linked statically. ArgyllCMS is installed by the existing portable backend when required."),
   Page::About=>div().flex().flex_col().gap_3().child(heading("ICC Profile Studio","Experimental GPUI frontend")).child(format!("Core version {}",env!("CARGO_PKG_VERSION"))).child("Developed by emadgh. LittleCMS 2 · ArgyllCMS · Rust.").child("Updater integration is pending in this frontend. Use the stable executable for updates."),
   other=>div().flex().flex_col().gap_3().child(heading(PAGES.iter().find(|(p,_)|*p==other).map(|(_,n)|*n).unwrap_or("Tool"),"Planned module")).child("Not implemented yet. See ROADMAP.md."),
  };
  div().flex().size_full().bg(rgb(0x152031)).text_color(rgb(0xe2e8f0)).child(sidebar)
   .child(div().flex().flex_col().flex_1().min_w(px(0.)).h_full()
    .child(div().flex().items_center().justify_between().px_5().py_3().bg(rgb(0x1b2637)).child(self.source.as_ref().and_then(|p|p.file_name()).map(|p|p.to_string_lossy().into_owned()).unwrap_or("No profile selected".into())).child(button("toggle-preview",if self.preview_open{"Hide preview"}else{"Show preview"}).on_click(cx.listener(|this,_,_,cx|{this.preview_open=!this.preview_open;if this.preview_open&&this.preview.is_none(){this.refresh_gamut(cx);}cx.notify();}))))
    .child(div().id("content-scroll").flex_1().overflow_y_scroll().p_5().child(content))
    .child(div().px_5().py_3().bg(rgb(0x101a2a)).text_sm().child(self.status.clone())))
   .when(self.preview_open,|root|root.child(self.preview_panel(cx)))
 }
}
fn main(){
 Application::new().run(|cx:&mut App|{
  let bounds=Bounds::centered(None,size(px(1440.),px(880.)),cx);
  cx.open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(bounds)),..Default::default()},|_,cx|cx.new(|_|Studio::default())).expect("Open Studio window");
  cx.activate(true);
 });
}
