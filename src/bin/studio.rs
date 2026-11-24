#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use gpui::{prelude::*, *};
use color_convert_rust::{Profile, backend::{self,Job,Mode}, inspection::{self,Inspection}};
use std::{path::PathBuf,sync::mpsc,time::Duration};
use color_convert_rust::comparison::{self,Comparison};
use color_convert_rust::editing::{self,Edit};
use color_convert_rust::correction::{Operation,Selective,BlackGeneration,IDENTITY_POINTS};
#[path = "../gamut_view.rs"] mod gamut_view;
use color_convert_rust::gamut::{self,Overlay};
use gamut_view::ViewOptions;
use color_convert_rust::images::{self,Options as ImageOptions,Output as ImageOutput};
use color_convert_rust::streaming::{self,StreamOptions};
use std::sync::{Arc,atomic::{AtomicBool,Ordering}};
use color_convert_rust::gradient::{self,GradientReport};
use color_convert_rust::{library,reports,preferences::{self,Preferences}};
#[cfg(windows)] use update_via_github::{UpdateConfig,UpdateManager,UpdateStatus};
use color_convert_rust::{optimizer::{self,Settings as RecipeSettings,Report as RecipeReport},lab::{self,MeasuredComparison}};

const STUDIO_VERSION:&str=include_str!("../../STUDIO_VERSION");

#[derive(Clone,Copy,PartialEq,Eq)]
enum Page { Dashboard, Library, Inspector, Validator, Rebuilder, Comparison, Curves, Reorder, Metadata, Gamut, Gradient, Correction, Smoothing, Converter, DeviceLink, Proof, Recipes, Black, Lab, Settings, About }
const PAGES: [(Page,&str);21] = [
 (Page::Dashboard,"Dashboard"),(Page::Library,"Profile Library"),(Page::Inspector,"Profile Inspector"),(Page::Validator,"Profile Validator"),(Page::Rebuilder,"Profile Rebuilder"),
 (Page::Comparison,"Profile Comparison"),(Page::Curves,"Tone Curves"),(Page::Reorder,"Channel Reorder"),(Page::Metadata,"Profile Metadata"),(Page::Gamut,"Gamut Viewer"),(Page::Gradient,"Gradient Analyzer"),(Page::Correction,"Color Correction"),(Page::Smoothing,"LUT Smoothing"),(Page::Converter,"Image Converter"),(Page::DeviceLink,"DeviceLink"),(Page::Proof,"Soft Proof"),(Page::Recipes,"Ink Optimizer"),(Page::Black,"Black Generation"),(Page::Lab,"Profiling Lab"),(Page::Settings,"Settings"),(Page::About,"About & Updates")
];
enum Event { Progress(String), Done(Result<String,String>) }
struct Studio {
 page:Page, source:Option<PathBuf>, profile:Option<Profile>, report:Option<Inspection>, error:Option<String>,
 caps:[f64;4],tac:f64,grid:usize,mode:Mode,status:String,busy:bool, candidate:Option<PathBuf>,comparison:Option<Comparison>,compare_error:Option<String>,compare_grid:usize,compare_intent:lcms2::Intent,gamma:[f64;4],order:[usize;4], preview_open:bool, preview:Option<Overlay>, preview_revision:u64, preview_cache:std::cell::RefCell<gamut_view::PreviewCache>, preview_options:ViewOptions, gamut_grid:usize,gradient:Option<GradientReport>,gradient_error:Option<String>,coverage_threshold:f64, selective:[f64;5], correction_grid:usize, correction_tac:f64, smoothing:f64, smoothing_passes:usize,orbit_last:Option<Point<Pixels>>,image_files:Vec<PathBuf>,image_root:Option<PathBuf>,image_options:ImageOptions,image_results:Vec<ImageOutput>,image_errors:Vec<String>,image_preview:usize,proof_source:Option<PathBuf>,monitor_profile:Option<PathBuf>,proof_media:bool,proof_bpc:bool,proof_output:Option<PathBuf>,stream_options:StreamOptions,stream_enabled:bool,batch_cancel:Option<Arc<AtomicBool>>,recipe_target:[f64;3],recipe_settings:RecipeSettings,recipe_result:Option<RecipeReport>,lab_levels:usize,measured:[Option<PathBuf>;2],measurement_result:Option<MeasuredComparison>,black:BlackGeneration,curve_points:[[f64;5];4],library_root:Option<PathBuf>,library_entries:Vec<library::Entry>,logo:std::sync::Arc<gpui::Image>,#[cfg(windows)] updater:UpdateManager,
}
impl Default for Studio {
 fn default()->Self{Self{page:Page::Dashboard,source:None,profile:None,report:None,error:None,caps:[100.,100.,100.,0.],tac:240.,grid:33,mode:Mode::Custom,status:"Open an ICC or ICM profile to start.".into(),busy:false,candidate:None,comparison:None,compare_error:None,compare_grid:17,compare_intent:lcms2::Intent::RelativeColorimetric,gamma:[1.;4],order:[0,1,2,3],preview_open:true,preview:None,preview_revision:0,preview_cache:std::cell::RefCell::new(gamut_view::PreviewCache::default()),preview_options:ViewOptions::default(),gamut_grid:17,gradient:None,gradient_error:None,coverage_threshold:2.,selective:[135.,45.,0.,0.,0.],correction_grid:33,correction_tac:400.,smoothing:0.20,smoothing_passes:1,orbit_last:None,image_files:vec![],image_root:None,image_options:ImageOptions::default(),image_results:vec![],image_errors:vec![],image_preview:0,proof_source:None,monitor_profile:None,proof_media:true,proof_bpc:false,proof_output:None,stream_options:StreamOptions::default(),stream_enabled:false,batch_cancel:None,recipe_target:[50.,0.,0.],recipe_settings:RecipeSettings::default(),recipe_result:None,lab_levels:5,measured:[None,None],measurement_result:None,black:BlackGeneration::default(),curve_points:IDENTITY_POINTS,library_root:None,library_entries:vec![],logo:std::sync::Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png,include_bytes!("../../assets/icon.png").to_vec())),#[cfg(windows)] updater:UpdateManager::new(UpdateConfig::new("emadgh/color-convert-rust","ICC-Profile-Studio.exe",STUDIO_VERSION.trim()).with_app_name("ICC Profile Studio").with_checksum_asset("ICC-Profile-Studio.exe.sha256").with_required_checksum(true))}}
}
fn button(id: impl Into<ElementId>,label:impl Into<SharedString>)->Stateful<Div>{
 div().id(id).px_3().py_2().rounded_md().bg(rgb(0x263449)).text_color(rgb(0xe6eefb)).cursor_pointer().hover(|s|s.bg(rgb(0x354963))).child(label.into())
}
fn heading(title:&str,sub:&str)->Div{div().flex().flex_col().gap_1().child(div().text_2xl().child(title.to_owned())).child(div().text_sm().text_color(rgb(0x94a3b8)).child(sub.to_owned()))}
fn neutral_plot(samples:&[comparison::NeutralSample],metric:usize)->impl IntoElement{
 let points:Vec<_>=samples.iter().map(|n|{
  let (a,b)=if metric==0{(n.reference_lab[0],n.candidate_lab[0])}else{(n.reference_ink[metric-1],n.candidate_ink[metric-1])};
  (n.rgb as f32/255.,(a/100.).clamp(0.,1.) as f32,(b/100.).clamp(0.,1.) as f32)
 }).collect();
 canvas(move|_,_,_|{},move|bounds,_,window,_|{
  window.paint_quad(fill(bounds,rgb(0x0b1220)));
  for i in 0..=4{
   let y=bounds.origin.y+(bounds.size.height-px(4.))*i as f32/4.;
   window.paint_quad(fill(Bounds{origin:point(bounds.origin.x,y),size:size(bounds.size.width,px(1.))},rgb(0x283a51)));
  }
  for (index,color) in [(1,0x60a5fa),(2,0xfbbf24)]{
   for pair in points.windows(2){
    let a=pair[0];let b=pair[1];let (ay,by)=if index==1{(a.1,b.1)}else{(a.2,b.2)};
    for i in 0..=24{
     let t=i as f32/24.;let x=a.0+(b.0-a.0)*t;let y=ay+(by-ay)*t;
     let p=point(bounds.origin.x+px(2.)+(bounds.size.width-px(4.))*x,bounds.origin.y+px(2.)+(bounds.size.height-px(4.))*(1.-y));
     window.paint_quad(fill(Bounds{origin:p,size:size(px(1.6),px(1.6))},rgb(color)));
    }
   }
  }
 }).w_full().h(px(92.))
}

impl Studio {
 fn open(&mut self,cx:&mut Context<Self>) {
  if self.busy{return}
  let Some(path)=rfd::FileDialog::new().add_filter("ICC profile",&["icc","icm"]).pick_file() else{return};
  self.load_reference(path,cx);
 }
 fn load_reference(&mut self,path:PathBuf,cx:&mut Context<Self>){
  if self.busy{return}
  self.source=Some(path.clone());self.black=BlackGeneration::default();self.curve_points=IDENTITY_POINTS;self.recipe_result=None;self.selective=[135.,45.,0.,0.,0.];self.correction_grid=33;self.correction_tac=400.;self.smoothing=0.20;self.smoothing_passes=1;self.preview=None;self.candidate=None;self.gradient=None;self.gradient_error=None;self.gamma=[1.;4];self.order=[0,1,2,3];self.comparison=None;self.compare_error=None;self.report=None;self.profile=None;self.error=None;
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
  let grid=self.compare_grid;let intent=self.compare_intent;
  self.busy=true;self.comparison=None;self.compare_error=None;self.status="Comparing profile predictions with LittleCMS...".into();cx.notify();
  cx.spawn(async move |this,cx|{
   let result=cx.background_spawn(async move{comparison::compare(&a,&b,grid,intent)}).await;
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
   .child(div().flex().flex_wrap().gap_1().children([("Relative",lcms2::Intent::RelativeColorimetric),("Perceptual",lcms2::Intent::Perceptual),("Saturation",lcms2::Intent::Saturation),("Absolute",lcms2::Intent::AbsoluteColorimetric)].into_iter().enumerate().map(|(i,(n,intent))|button(("compare-intent",i),if self.compare_intent==intent{format!("{n} ✓")}else{n.to_owned()}).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.compare_intent=intent;v.comparison=None;v.compare_error=None;cx.notify();}})))))
   .child(button("compare-run",if self.busy{"Working…"}else{"Compare profiles"}).on_click(cx.listener(|this,_,_,cx|this.compare_profiles(cx))));
  if let Some(e)=&self.compare_error{p=p.child(div().text_color(rgb(0xfca5a5)).child(e.clone()));}
  if let Some(r)=&self.comparison{
   p=p.child(button("export-comparison","Export analysis CSV").on_click(cx.listener(|v,_,_,cx|{if !v.busy{if let Some(r)=&v.comparison{if let Some(path)=rfd::FileDialog::new().add_filter("CSV",&["csv"]).set_file_name("comparison.csv").save_file(){v.status=reports::comparison(&path,r).map(|_|format!("Saved {}",path.display())).unwrap_or_else(|e|format!("Error: {e}"));cx.notify();}}}})));
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
    .child(div().text_lg().child("Neutral axis — all 33 samples"))
    .child("Blue = before; amber = after. L* and ink bars use fixed 0–100 scales.")
    .child(neutral_plot(&r.neutral,0))
    .child("L* (input gray 0 → 255)");
   for j in 0..4{p=p.child(neutral_plot(&r.neutral,j+1)).child(format!("Ink {} (%)",j+1));}
   for n in &r.neutral{
    p=p.child(div().p_2().rounded_md().bg(rgb(0x1b2637))
     .child(format!("Gray {:3} | ΔE00 {:.3} | L*a*b* {:.2?} → {:.2?}",n.rgb,n.delta_e,n.reference_lab,n.candidate_lab))
     .child(format!("Ink % {:.2?} → {:.2?}",n.reference_ink,n.candidate_ink)));
   }
   p=p.child(format!("Separation intent: {:?}; predicted appearance evaluated media-relative D50; no BPC. Predicted differences only, not measured print, physical gamut or production ink averages.",r.intent));
  }
  p
 }
 fn save_edit(&mut self,kind:u8,cx:&mut Context<Self>){
  if self.busy{return}let Some(source)=self.source.clone()else{return};
  let suffix=match kind{0=>"curves",1=>"reordered",3=>"selective",4=>"smoothed",5=>"ink_optimized",6=>"black_generation",7=>"point_curves",_=>"renamed"};
  let default_name=format!("{}_{}.icc",source.file_stem().unwrap_or_default().to_string_lossy(),suffix);
  let Some(mut destination)=rfd::FileDialog::new().add_filter("ICC profile",&["icc","icm"]).set_file_name(default_name).save_file()else{return};
  if destination.extension().is_none(){destination.set_extension("icc");}
  let output_path=destination.clone();
  let edit=match kind{0=>Edit::Tone(self.gamma),1=>Edit::Reorder(self.order),3=>Edit::Correction(Operation::Selective(Selective{center:self.selective[0],width:self.selective[1],light:self.selective[2],chroma_percent:self.selective[3],hue_shift:self.selective[4]}),self.correction_grid,self.correction_tac),4=>Edit::Correction(Operation::Smooth{strength:self.smoothing,passes:self.smoothing_passes},self.correction_grid,self.correction_tac),5=>Edit::Correction(Operation::Optimize{weights:self.recipe_settings.weights,tolerance:self.recipe_settings.tolerance},self.correction_grid.min(33),self.correction_tac),6=>Edit::Correction(Operation::Black(self.black),self.correction_grid.min(33),self.correction_tac),7=>Edit::Correction(Operation::Points(self.curve_points),self.correction_grid,self.correction_tac),_=>Edit::Rename(destination.file_stem().unwrap_or_default().to_string_lossy().into_owned())};
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
   .child(button("save-curves","Save gamma ICC").on_click(cx.listener(|this,_,_,cx|this.save_edit(0,cx))))
   .child(self.point_curves_page(cx))
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
    let status=match &result.reference{Ok(c)=>format!("Gamut ready: {} reference points{}",c.points.len(),match &result.candidate{Some(Ok(c))=>format!(", {} candidate points",c.points.len()),Some(Err(e))=>format!("; candidate failed: {e}"),None=>String::new()}),Err(e)=>format!("Reference gamut failed: {e}")};view.preview=Some(result);view.preview_revision=view.preview_revision.wrapping_add(1);view.busy=false;view.status=status;cx.notify();
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
    .child(button("show-after",if o.candidate{"After ✓"}else{"After"}).on_click(cx.listener(|this,_,_,cx|{this.preview_options.candidate=!this.preview_options.candidate;cx.notify();}))));
   p=p.child(div().id("gamut-orbit").w_full().flex_shrink_0().cursor_pointer()
    .on_mouse_down(MouseButton::Left,cx.listener(|this,e:&MouseDownEvent,_,cx|{this.orbit_last=Some(e.position);cx.notify();}))
    .on_mouse_up(MouseButton::Left,cx.listener(|this,_,_,cx|{this.orbit_last=None;cx.notify();}))
    .on_mouse_move(cx.listener(|this,e:&MouseMoveEvent,_,cx|{
     if !e.dragging(){this.orbit_last=None;return}
     if let Some(last)=this.orbit_last{
      if this.preview_options.three_d{
       let dx=f32::from(e.position.x-last.x);let dy=f32::from(e.position.y-last.y);
       // Ignore abrupt jumps when the cursor returns from outside the plot.
       if dx.abs()<100.&&dy.abs()<100.{
        this.preview_options.yaw=(this.preview_options.yaw+dx*0.01).rem_euclid(std::f32::consts::TAU);
        this.preview_options.pitch=(this.preview_options.pitch+dy*0.01).clamp(-1.55,1.55);cx.notify();
       }
      }
      this.orbit_last=Some(e.position);
     }
    }))
    .on_scroll_wheel(cx.listener(|this,e:&ScrollWheelEvent,_,cx|{
     let dy=f32::from(e.delta.pixel_delta(px(20.)).y);
     if dy.is_finite(){this.preview_options.zoom=(this.preview_options.zoom*(dy*0.004).clamp(-0.6,0.6).exp()).clamp(0.5,3.);cx.notify();cx.stop_propagation();}
    }))
    .child(gamut_view::plot(self.preview.as_ref(),self.preview_revision,o,&mut self.preview_cache.borrow_mut())));
  p=p
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
  { 
   if let Some(data)=&self.preview{
   match &data.reference{Ok(c)=>p=p.child(format!("Before: {} distinct / {} targets",c.points.len(),c.sampled)),Err(e)=>p=p.child(format!("Before failed: {e}"))}
   if let Some(c)=&data.candidate{match c{Ok(c)=>p=p.child(format!("After: {} distinct / {} targets",c.points.len(),c.sampled)),Err(e)=>p=p.child(format!("After failed: {e}"))}}
   }
   p=p.child(button("mesh-toggle",if o.wireframe{"Wireframe ✓"}else{"Point cloud"}).on_click(cx.listener(|this,_,_,cx|{this.preview_options.wireframe=!this.preview_options.wireframe;cx.notify();})));
   p=p.child("Drag to orbit (3D); scroll to zoom. Both profiles share the same auto-fit scale.")
    .child(button("save-gamut-png","Export current gamut PNG").on_click(cx.listener(|v,_,_,cx|{if let Some(data)=&v.preview{if let Some(path)=rfd::FileDialog::new().add_filter("PNG",&["png"]).set_file_name("gamut.png").save_file(){let result=color_convert_rust::gamut_raster::render(Some(data),v.preview_options).png().and_then(|bytes|{use std::io::Write;let mut f=tempfile::NamedTempFile::new_in(path.parent().unwrap_or(std::path::Path::new("."))).map_err(|e|e.to_string())?;f.write_all(&bytes).map_err(|e|e.to_string())?;f.persist_noclobber(&path).map_err(|e|e.to_string())?;Ok(())});v.status=result.map(|_|format!("Saved {}",path.display())).unwrap_or_else(|e|format!("Error: {e}"));cx.notify();}}})));
   if o.wireframe{p=p.child("Convex hull wireframe / 2D outline; approximate envelope.");}
   if o.three_d{p=p.child("Axes: red +a*, green +b*, white +L*");}else{p=p.child(if o.slice{format!("a* / b* | L* {:.0} ± 5",o.light)}else{"a* / b* | all lightness values".into()});}
  }
  if self.preview.is_none(){p=p.child(if self.busy{"Sampling in progress…"}else{"Open an ICC or refresh to calculate the preview."});}
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
   p=p.child(button("export-gradients","Export gradient CSV").on_click(cx.listener(|v,_,_,cx|{if !v.busy{if let Some(r)=&v.gradient{if let Some(path)=rfd::FileDialog::new().add_filter("CSV",&["csv"]).set_file_name("gradients.csv").save_file(){v.status=reports::gradient(&path,r).map(|_|format!("Saved {}",path.display())).unwrap_or_else(|e|format!("Error: {e}"));cx.notify();}}}})));
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
 fn correction_options(&self,cx:&mut Context<Self>)->Div{
  let estimate=self.profile.as_ref().and_then(|p|p.estimated_tac_percent());
  let mut p=div().flex().flex_col().gap_2()
   .child(div().flex().items_center().gap_1().child("CLUT grid").children([17usize,33,49,65].map(|g|button(("edit-grid",g),if self.correction_grid==g{format!("{g} ✓")}else{g.to_string()}).on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.correction_grid=g;cx.notify();}})))))
   .child(div().flex().items_center().gap_2().child(format!("TAC ceiling: {:.0}%",self.correction_tac))
    .child(button("edit-tac-minus","−5").on_click(cx.listener(|this,_,_,cx|{if !this.busy{this.correction_tac=(this.correction_tac-5.).max(0.);cx.notify();}})))
    .child(button("edit-tac-plus","+5").on_click(cx.listener(|this,_,_,cx|{if !this.busy{this.correction_tac=(this.correction_tac+5.).min(400.);cx.notify();}}))));
  if let Some(v)=estimate{p=p.child(button("edit-tac-reference",format!("Use reference B2A1 node estimate ({v:.0}%)")).on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.correction_tac=v;cx.notify();}})));}
  p.child("Default 400% means no added TAC reduction; select a safe explicit limit. Source channel maxima are enforced automatically. A2B is preserved.")
 }
 fn correction_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Selective Color Correction","Soft hue selection in D50 Lab/LCh; independent B2A intents."));
  let Some(profile)=&self.profile else{return p.child("Open a compatible ICC v4 CMYK printer profile.")};
  if !profile.cmyk()||profile.version_major()!=4||profile.pcs()!="Lab "{return p.child("Initial editor supports ICC v4 CMYK / Lab with simple mBA LUTs only.")}
  p=p.child(div().flex().gap_1().flex_wrap().children([("Red",40.,0xc34c4c),("Yellow",90.,0xdbbb35),("Green",135.,0x47a86a),("Cyan",200.,0x2fa6b8),("Blue",270.,0x547cda),("Magenta",320.,0xb865b5)].into_iter().enumerate().map(|(i,(name,v,color))|button(("hue-preset",i),name).bg(rgb(color)).text_color(rgb(0xffffff)).on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.selective[0]=v;cx.notify();}})))));
  for (i,(name,lo,hi,step,unit)) in [("Hue center",0.,360.,5.,"°"),("Hue half-width",10.,180.,5.,"°"),("Lightness",-30.,30.,1.," L*"),("Chroma",-80.,80.,5.,"%"),("Hue shift",-60.,60.,5.,"°")].into_iter().enumerate(){
   p=p.child(div().flex().items_center().gap_2().child(div().w(px(120.)).child(name))
    .child(button(("correction-minus",i),"−").on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.selective[i]=(this.selective[i]-step).max(lo);cx.notify();}})))
    .child(div().w(px(90.)).child(format!("{:.0}{unit}",self.selective[i])))
    .child(button(("correction-plus",i),"+").on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.selective[i]=(this.selective[i]+step).min(hi);cx.notify();}}))));
  }
  p.child(button("reset-selective","Reset").on_click(cx.listener(|this,_,_,cx|{if !this.busy{this.selective=[135.,45.,0.,0.,0.];cx.notify();}})))
   .child(self.correction_options(cx))
   .child("Near-neutrals and black/white endpoints are protected. Corrects the requested PCS color before separation, not measured print color. Resampling can alter colors outside the selected band.")
   .child(button("save-selective","Generate corrected ICC").on_click(cx.listener(|this,_,_,cx|this.save_edit(3,cx))))
 }
 fn smoothing_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("LUT Smoothing","Bounded six-neighbor averaging in the B2A separation lattice."));
  let Some(profile)=&self.profile else{return p.child("Open a compatible ICC v4 CMYK printer profile.")};
  if !profile.cmyk()||profile.version_major()!=4||profile.pcs()!="Lab "{return p.child("Initial editor supports ICC v4 CMYK / Lab with simple mBA LUTs only.")}
  p=p.child(div().flex().items_center().gap_2().child(format!("Strength: {:.0}%",self.smoothing*100.))
   .child(button("smooth-minus","−5").on_click(cx.listener(|this,_,_,cx|{if !this.busy{this.smoothing=(this.smoothing-0.05).max(0.05);cx.notify();}})))
   .child(button("smooth-plus","+5").on_click(cx.listener(|this,_,_,cx|{if !this.busy{this.smoothing=(this.smoothing+0.05).min(0.5);cx.notify();}}))))
   .child(div().flex().gap_1().items_center().child("Passes").children([1usize,2,3,4,5].map(|n|button(("smooth-pass",n),if n==self.smoothing_passes{format!("{n} ✓")}else{n.to_string()}).on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.smoothing_passes=n;cx.notify();}})))));
  p.child(self.correction_options(cx))
   .child("Lattice boundaries are not averaged; initial resampling and ink constraints can still change them. Use Gradient Analyzer and Profile Comparison after saving to check the trade-off between smoothness and color accuracy.")
   .child(button("save-smoothing","Generate smoothed ICC").on_click(cx.listener(|this,_,_,cx|this.save_edit(4,cx))))
 }
 fn convert_images(&mut self,cx:&mut Context<Self>){
  if self.busy{return}let (Some(reference),Some(root))=(self.source.clone(),self.image_root.clone())else{self.status="Choose reference ICC and output folder.".into();cx.notify();return};
  if self.image_files.is_empty(){return}
  let dest=self.candidate.clone().unwrap_or_else(||reference.clone());let files=self.image_files.clone();let options=self.image_options;let stream=self.stream_options;let streaming_enabled=self.stream_enabled;let cancel=Arc::new(AtomicBool::new(false));self.batch_cancel=Some(cancel.clone());self.image_preview=0;
  self.busy=true;self.image_results.clear();self.image_errors.clear();self.status="Converting image batch with LittleCMS…".into();cx.notify();
  cx.spawn(async move|this,cx|{
   enum BatchEvent{Progress(String),Result(PathBuf,Result<ImageOutput,String>)}
   let (tx,rx)=mpsc::channel();let worker_cancel=cancel.clone();
   std::thread::spawn(move||{
    for path in files{
     if worker_cancel.load(Ordering::Relaxed){break}
     let is_tiff=path.extension().and_then(|s|s.to_str()).is_some_and(|s|s.eq_ignore_ascii_case("tif")||s.eq_ignore_ascii_case("tiff"));
     let result=if streaming_enabled&&is_tiff{
      streaming::convert(&path,&reference,&dest,&root,options,stream,&worker_cancel,|i,n|{if i==1||i==n||i%8==0{let _=tx.send(BatchEvent::Progress(format!("{}: strip {i}/{n}",path.display())));}})
     }else{images::convert(&path,&reference,&dest,&root,options)};
     let _=tx.send(BatchEvent::Result(path,result));
    }
   });
   loop{
    match rx.try_recv(){
     Ok(BatchEvent::Progress(s))=>{if this.update(cx,|v,cx|{v.status=s;cx.notify();}).is_err(){break}},
     Ok(BatchEvent::Result(path,result))=>{if this.update(cx,|v,cx|{match result{Ok(r)=>v.image_results.push(r),Err(e)=>v.image_errors.push(format!("{}: {e}",path.display()))}v.status=format!("Converted {} / failed {}",v.image_results.len(),v.image_errors.len());cx.notify();}).is_err(){break}},
     Err(mpsc::TryRecvError::Disconnected)=>{let _=this.update(cx,|v,cx|{v.busy=false;v.batch_cancel=None;v.status=format!("Batch {}: {} succeeded, {} failed.",if cancel.load(Ordering::Relaxed){"cancelled"}else{"finished"},v.image_results.len(),v.image_errors.len());cx.notify();});break},
     Err(mpsc::TryRecvError::Empty)=>cx.background_executor().timer(Duration::from_millis(70)).await,
    }
   }
  }).detach();
 }
 fn image_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Image Processing Studio","RGB PNG/JPEG, RGB/CMYK TIFF → tagged CMYK16 TIFF; batch processing."))
   .child(button("image-files","Select images").on_click(cx.listener(|v,_,_,cx|{if !v.busy{if let Some(p)=rfd::FileDialog::new().add_filter("Images",&["png","jpg","jpeg","tif","tiff"]).pick_files(){v.image_files=p;v.image_results.clear();v.image_errors.clear();cx.notify();}}})))
   .child(format!("{} image(s) selected",self.image_files.len()))
   .child(button("image-root","Select output directory").on_click(cx.listener(|v,_,_,cx|{if !v.busy{if let Some(p)=rfd::FileDialog::new().pick_folder(){v.image_root=Some(p);cx.notify();}}})))
   .child(self.image_root.as_ref().map(|p|p.display().to_string()).unwrap_or("No output folder".into()))
   .child(format!("Destination: {}",self.candidate.as_ref().or(self.source.as_ref()).map(|p|p.display().to_string()).unwrap_or("Select a printer ICC".into())))
   .child(button("assume-icc",if self.image_options.assume_untagged{"Untagged: assume sRGB / reference CMYK ✓"}else{"Untagged: reject (click to explicitly assume)"}).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.image_options.assume_untagged=!v.image_options.assume_untagged;cx.notify();}})))
   .child(div().flex().gap_1().flex_wrap().children([("Relative",lcms2::Intent::RelativeColorimetric),("Perceptual",lcms2::Intent::Perceptual),("Saturation",lcms2::Intent::Saturation),("Absolute",lcms2::Intent::AbsoluteColorimetric)].into_iter().enumerate().map(|(i,(n,intent))|button(("image-intent",i),if self.image_options.intent==intent{format!("{n} ✓")}else{n.into()}).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.image_options.intent=intent;cx.notify();}})))))
   .child(div().flex().gap_1().items_center().child(format!("DPI {}",self.image_options.dpi))
    .child(button("dpi-down","−10").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.image_options.dpi=v.image_options.dpi.saturating_sub(10).max(50);cx.notify();}})))
    .child(button("dpi-up","+10").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.image_options.dpi=(v.image_options.dpi+10).min(2400);cx.notify();}}))))
   .child(div().flex().gap_1().items_center().child(format!("Reject above TAC {:.0}%",self.image_options.tac))
    .child(button("image-tac-down","−5").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.image_options.tac=(v.image_options.tac-5.).max(0.);cx.notify();}})))
    .child(button("image-tac-up","+5").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.image_options.tac=(v.image_options.tac+5.).min(400.);cx.notify();}}))))
   .child(button("stream-mode",if self.stream_enabled{"Stream interleaved strip TIFF ✓"}else{"In-memory conversion (24 MP)"}).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.stream_enabled=!v.stream_enabled;cx.notify();}})))
   .child(button("bigtiff-mode",if self.stream_options.bigtiff{"BigTIFF ✓ (verify RIP support)"}else{"Classic TIFF output"}).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.stream_options.bigtiff=!v.stream_options.bigtiff;cx.notify();}})))
   .child(button("full-separations",if self.stream_options.separations{"Full-resolution 16-bit separations ✓"}else{"Thumbnail separations only"}).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.stream_options.separations=!v.stream_options.separations;cx.notify();}})))
   .child("BigTIFF and full-size separations apply only to streaming TIFF. Strips ≤ 2 MP each, total ≤ 2 billion pixels. Tiled, planar, multipage and alpha TIFF not supported by this path. PNG/JPEG or disabled streaming remains limited to 24 MP. Set physical DPI deliberately; source metadata is not copied.")
   .child(button("cancel-batch","Cancel batch").on_click(cx.listener(|v,_,_,cx|{if let Some(c)=&v.batch_cancel{c.store(true,Ordering::Relaxed);v.status="Cancellation requested; current non-streamed file may finish.".into();cx.notify();}})))
   .child(button("convert-batch",if self.busy{"Working…"}else{"Convert selected images"}).on_click(cx.listener(|v,_,_,cx|v.convert_images(cx))));
  for e in &self.image_errors{p=p.child(div().text_color(rgb(0xfca5a5)).child(e.clone()));}
  for (i,r) in self.image_results.iter().enumerate(){
   p=p.child(div().p_3().rounded_md().bg(rgb(0x1b2637)).flex().flex_col().gap_1().child(r.directory.display().to_string()).child(format!("{} × {} · peak TAC {:.2}% · mean inks {:?}",r.width,r.height,r.peak_tac,r.mean))
   .child(button(("pick-image-result",i),"Show image previews").on_click(cx.listener(move|v,_,_,cx|{v.image_preview=i;cx.notify();}))));
  }
  if let Some(r)=self.image_results.get(self.image_preview){
   p=p.child(div().text_lg().child("Before / after — relative sRGB simulation"))
    .child(div().flex().gap_2().children(["before.png","after.png"].map(|n|div().flex_1().min_w(px(0.)).child(n).child(img(r.directory.join(n)).w_full().h(px(200.))))))
    .child(div().text_lg().child("Channel separations"))
    .child(div().flex().gap_1().children((1..=4).map(|j|div().flex_1().min_w(px(0.)).child(format!("Ink {j}")).child(img(r.directory.join(format!("channel_{j}.png"))).w_full().h(px(100.))))))
    .child("TAC coverage: blue = low, red = high").child(img(r.directory.join("coverage.png")).w_full().h(px(220.)));
  }
  p
 }
 fn link_page(&self,cx:&mut Context<Self>)->Div{
  div().flex().flex_col().gap_3().child(heading("DeviceLink Generator","LittleCMS CMYK → CMYK ICC v4.3."))
   .child("Reference is source CMYK and candidate is destination. Select the candidate in Gamut Preview or Profile Comparison. Relative colorimetric; no special black preservation or certified channel cap guarantee.")
   .child(button("save-link","Generate DeviceLink").on_click(cx.listener(|v,_,_,cx|{
    if v.busy{return}let (Some(a),Some(b))=(v.source.clone(),v.candidate.clone())else{v.status="Select two printer profiles.".into();cx.notify();return};
    let Some(mut dest)=rfd::FileDialog::new().add_filter("ICC",&["icc"]).set_file_name("CMYK_to_CMYK.icc").save_file()else{return};if dest.extension().is_none(){dest.set_extension("icc");}
    v.busy=true;v.status="Generating DeviceLink…".into();cx.notify();
    cx.spawn(async move|this,cx|{
     let result=cx.background_spawn(async move{images::device_link(&a,&b,&dest,lcms2::Intent::RelativeColorimetric).map(|r|format!("Saved {} | {} samples | Max link/direct channel deviation {:.3} pp | Peak sampled TAC {:.2}% | Max channels {:.2?}%",dest.display(),r.sample_count,r.max_channel_difference,r.peak_tac,r.maxima))}).await;
     let _=this.update(cx,|v,cx|{v.busy=false;v.status=result.unwrap_or_else(|e|format!("Error: {e}"));cx.notify();});
    }).detach();
   })))
 }
 fn recipe_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Ink / Cost Optimizer","Find a bounded device recipe for a D50 Lab target."))
   .child("Uses the reference A2B prediction. Relative ink weights are cost proxies, not actual mass or currency. Search is approximate and can fail to find a feasible recipe.");
  for (i,(name,lo,hi)) in [("L*",0.,100.),("a*",-128.,127.),("b*",-128.,127.)].into_iter().enumerate(){
   p=p.child(div().flex().items_center().gap_2().child(format!("{name}: {:.1}",self.recipe_target[i]))
    .child(button(("recipe-target-minus",i),"−1").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.recipe_target[i]=(v.recipe_target[i]-1.).max(lo);v.recipe_result=None;cx.notify();}})))
    .child(button(("recipe-target-plus",i),"+1").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.recipe_target[i]=(v.recipe_target[i]+1.).min(hi);v.recipe_result=None;cx.notify();}}))));
  }
  for j in 0..4{
   p=p.child(div().flex().items_center().gap_2().child(format!("Ink {} weight {:.2}",j+1,self.recipe_settings.weights[j]))
    .child(button(("weight-minus",j),"−0.25").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.recipe_settings.weights[j]=(v.recipe_settings.weights[j]-0.25).max(0.01);v.recipe_result=None;cx.notify();}})))
    .child(button(("weight-plus",j),"+0.25").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.recipe_settings.weights[j]=(v.recipe_settings.weights[j]+0.25).min(100.);v.recipe_result=None;cx.notify();}}))));
  }
  p=p.child(format!("Caps and TAC from Rebuilder settings: {:?} / {:.0}%",self.caps,self.tac))
   .child(div().flex().gap_1().items_center().child("ΔE00 tolerance").children([1usize,2,3,5].map(|n|button(("recipe-tolerance",n),if self.recipe_settings.tolerance==n as f64{format!("{n} ✓")}else{n.to_string()}).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.recipe_settings.tolerance=n as f64;v.recipe_result=None;cx.notify();}})))))
   .child(button("solve-recipe","Find ink recipes").on_click(cx.listener(|v,_,_,cx|{
    if v.busy{return}let Some(p)=v.source.clone()else{return};let target=v.recipe_target;let mut s=v.recipe_settings;s.caps=v.caps;s.tac=v.tac;
    v.busy=true;v.recipe_result=None;v.status="Searching feasible ink recipes…".into();cx.notify();
    cx.spawn(async move|this,cx|{
     let result=cx.background_spawn(async move{optimizer::find(&p,target,s)}).await;
     let _=this.update(cx,|v,cx|{v.busy=false;match result{Ok(r)=>{v.status=format!("{} candidate recipes evaluated",r.evaluated);v.recipe_result=Some(r)},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
    }).detach();
   })));
  if let Some(r)=&self.recipe_result{
   for (label,recipe) in [("Original separation",Some(&r.baseline)),("Closest feasible",Some(&r.closest)),("Lowest cost found within tolerance",r.cheapest.as_ref())]{
    p=p.child(div().p_3().rounded_md().bg(rgb(0x1b2637)).flex().flex_col().gap_1().child(label).child(match recipe{Some(v)=>format!("Ink {:.2?}% · Lab {:.2?} · ΔE00 {:.3} · cost index {:.2}",v.ink,v.lab,v.delta_e,v.cost),None=>"No solution found within tolerance".into()}));
   }
  }
  p.child(div().text_lg().child("Whole-profile ink optimization (ICC v4 / mBA)"))
   .child(self.correction_options(cx))
   .child("Searches for lower weighted ink cost within the chosen ΔE00 of the original predicted appearance at each sampled LUT node, using A2B1. Uses grid 17 or 33; it does not certify interpolation, global color accuracy, real ink mass or savings. Source channel bounds preserved; refuses baseline exceeding selected TAC. Run Profile Comparison and Gradient Analyzer afterwards.")
   .child(button("optimize-profile","Generate ink-optimized ICC").on_click(cx.listener(|v,_,_,cx|v.save_edit(5,cx))))
 }
 fn lab_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Ceramic Profiling Lab","Direct-ink test charts, measured patch comparison and ArgyllCMS profiling."))
   .child("Record printer, glaze, ink batch, firing schedule and measurement conditions externally. Use identical measurement geometry and D50 Lab for both charts.")
   .child(div().text_lg().child("Generate device test chart"))
   .child(div().flex().gap_1().items_center().child("Levels per ink").children([3usize,5,7].map(|n|button(("chart-level",n),if self.lab_levels==n{format!("{n} ✓")}else{n.to_string()}).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.lab_levels=n;cx.notify();}})))))
   .child(format!("Rebuilder caps {:?}, TAC {:.0}%; DPI {} (Image Converter setting). 8 mm patches, 2 mm gaps.",self.caps,self.tac,self.image_options.dpi))
   .child(button("generate-chart","Export TIFF chart and measurement CSV").on_click(cx.listener(|v,_,_,cx|{
    if v.busy{return}let Some(p)=v.source.clone()else{return};let Some(root)=rfd::FileDialog::new().pick_folder()else{return};
    let (levels,caps,tac,dpi)=(v.lab_levels,v.caps,v.tac,v.image_options.dpi);
    v.busy=true;v.status="Generating device chart…".into();cx.notify();
    cx.spawn(async move|this,cx|{
     let result=cx.background_spawn(async move{lab::generate_chart(&p,&root,levels,caps,tac,dpi).map(|p|format!("Chart: {}",p.display()))}).await;
     let _=this.update(cx,|v,cx|{v.busy=false;v.status=result.unwrap_or_else(|e|format!("Error: {e}"));cx.notify();});
    }).detach();
   })))
   .child("Print the chart with no RIP color conversion; verify safe ink limits first. Fill blank measured L,a,b after firing; blank values are rejected.")
   .child(div().text_lg().child("Measured machine / glaze / firing / ink batch comparison"));
  for i in 0..2{p=p.child(button(("measured-file",i),if i==0{"Select measured baseline CSV"}else{"Select measured changed-condition CSV"}).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{if let Some(p)=rfd::FileDialog::new().add_filter("CSV",&["csv"]).pick_file(){v.measured[i]=Some(p);v.measurement_result=None;cx.notify();}}}))).child(self.measured[i].as_ref().map(|p|p.display().to_string()).unwrap_or("Not selected".into()));}
  p=p.child(button("compare-measured","Compare measurements").on_click(cx.listener(|v,_,_,cx|{
   if v.busy{return}let [Some(a),Some(b)]=v.measured.clone()else{return};v.busy=true;v.measurement_result=None;v.status="Comparing actual measurements…".into();cx.notify();
   cx.spawn(async move|this,cx|{
    let result=cx.background_spawn(async move{lab::compare(&a,&b)}).await;
    let _=this.update(cx,|v,cx|{v.busy=false;match result{Ok(r)=>{v.measurement_result=Some(r);v.status="Measured comparison finished".into()},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
   }).detach();
  })));
  if let Some(r)=&self.measurement_result{p=p.child(format!("{} patches | Mean ΔE {:.3} | P95 {:.3} | Max {:.3} ({}) | Mean ΔLab (after−before) {:.3?}",r.count,r.mean,r.p95,r.max,r.worst,r.bias));}
  p.child(div().text_lg().child("Profile measured printing condition (Argyll colprof)"))
   .child("Supply an actual CMYK .ti3 file from a measurement workflow. Builds a new ICC with explicit TAC and Argyll default black generation. Validate it before production. No automatic compensation or matching is assumed.")
   .child(button("colprof","Build ICC from measured TI3").on_click(cx.listener(|v,_,_,cx|{
    if v.busy{return}let Some(source)=rfd::FileDialog::new().add_filter("Measured Argyll TI3",&["ti3"]).pick_file()else{return};
    let Some(mut dest)=rfd::FileDialog::new().add_filter("ICC",&["icc"]).set_file_name("measured_profile.icc").save_file()else{return};if dest.extension().is_none(){dest.set_extension("icc");}
    let tac=v.tac;v.busy=true;v.status="Argyll colprof is building measured profile…".into();cx.notify();
    cx.spawn(async move|this,cx|{
     let result=cx.background_spawn(async move{lab::profile_from_ti3(&source,&dest,tac).map(|_|dest)}).await;
     let _=this.update(cx,|v,cx|{v.busy=false;match result{Ok(p)=>{v.status=format!("Saved {}",p.display());v.candidate=Some(p);v.preview=None;v.comparison=None;v.gradient=None;if v.preview_open{v.refresh_gamut(cx);}},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
    }).detach();
   })))
 }
 fn scan_library(&mut self,cx:&mut Context<Self>){
  if self.busy{return}let Some(root)=self.library_root.clone()else{return};
  self.busy=true;self.status="Scanning local profiles…".into();cx.notify();
  cx.spawn(async move|this,cx|{
   let result=cx.background_spawn(async move{library::scan(&root)}).await;
   let _=this.update(cx,|v,cx|{v.busy=false;match result{Ok(e)=>{v.status=format!("{} profiles in catalog",e.len());v.library_entries=e},Err(e)=>{v.status=format!("Error: {e}");v.library_entries.clear()}}cx.notify();});
  }).detach();
 }
 fn library_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Profile Library","Browse a local ICC folder with optional production metadata."))
   .child(button("choose-library","Select library folder").on_click(cx.listener(|v,_,_,cx|{if !v.busy{if let Some(root)=rfd::FileDialog::new().pick_folder(){v.library_root=Some(root);v.library_entries.clear();v.scan_library(cx);}}})))
   .child(self.library_root.as_ref().map(|v|v.display().to_string()).unwrap_or("No folder selected".into()))
   .child(button("rescan-library","Refresh library").on_click(cx.listener(|v,_,_,cx|v.scan_library(cx))))
   .child(button("manifest-library","Create profiles.csv metadata template").on_click(cx.listener(|v,_,_,cx|{
    if !v.busy{if let Some(root)=&v.library_root{v.status=library::write_manifest(root,&v.library_entries).map(|p|format!("Saved {}; edit metadata in your spreadsheet app, then refresh.",p.display())).unwrap_or_else(|e|format!("Error: {e}"));cx.notify();}}
   })))
   .child("Edit line, printer, glaze, ink batch, firing and notes in profiles.csv; refresh to read them. Manifest is never overwritten. Local files only.");
  for (i,e) in self.library_entries.iter().enumerate(){
   let a=e.path.clone();let b=e.path.clone();
   let mut row=div().p_2().rounded_md().bg(rgb(0x1b2637)).flex().flex_col().gap_1().child(e.meta.filename.clone()).child(e.format.clone())
    .child(format!("Line {} | Printer {} | Glaze {} | Ink {} | Firing {}",e.meta.line,e.meta.printer,e.meta.glaze,e.meta.ink_batch,e.meta.firing)).child(e.meta.notes.clone());
   if e.valid{row=row.child(div().flex().gap_2().child(button(("lib-ref",i),"Use reference").on_click(cx.listener(move|v,_,_,cx|v.load_reference(a.clone(),cx))))
     .child(button(("lib-candidate",i),"Use candidate").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.candidate=Some(b.clone());v.preview=None;v.comparison=None;v.gradient=None;v.gradient_error=None;if v.preview_open{v.refresh_gamut(cx);}cx.notify();}}))));}
   p=p.child(row);
  }p
 }
 #[cfg(windows)]
 fn update_poll(&self,cx:&mut Context<Self>){
  cx.spawn(async move|this,cx|{
   loop{
    cx.background_executor().timer(Duration::from_millis(150)).await;
    let state=this.update(cx,|v,cx|{cx.notify();matches!(v.updater.status(),UpdateStatus::Checking|UpdateStatus::Downloading{..})});
    if !matches!(state,Ok(true)){break}
   }
  }).detach();
 }
 fn about_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(div().flex().items_center().gap_3().child(img(self.logo.clone()).w(px(80.)).h(px(80.))).child(heading("ICC Profile Studio","Portable Windows color management / GPUI")))
   .child(format!("Studio {} · Core {} · Preview",STUDIO_VERSION.trim(),env!("CARGO_PKG_VERSION")))
   .child("Developed by emadgh. LittleCMS 2 · ArgyllCMS · Rust · GPUI · Parry3D.")
   .child(button("github-project","GitHub repository").on_click(cx.listener(|_,_,_,cx|cx.open_url("https://github.com/emadgh/color-convert-rust"))))
   .child(button("github-releases","Open signed-in GitHub Releases").on_click(cx.listener(|_,_,_,cx|cx.open_url("https://github.com/emadgh/color-convert-rust/releases"))))
   .child(button("github-builds","Windows preview builds").on_click(cx.listener(|_,_,_,cx|cx.open_url("https://github.com/emadgh/color-convert-rust/actions"))))
   .child(div().text_lg().child("Software updates"));
  #[cfg(windows)]{
   match self.updater.status(){
    UpdateStatus::Idle=>p=p.child("Check for a compatible Studio release."),
    UpdateStatus::Checking=>p=p.child("Checking GitHub Releases…"),
    UpdateStatus::UpToDate=>p=p.child("No newer accessible release found; private releases may be invisible."),
    UpdateStatus::Available(info)=>p=p.child(format!("Version {} available",info.version)).child(button("download-studio","Download verified update").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.updater.start_download();v.update_poll(cx);cx.notify();}}))),
    UpdateStatus::Downloading{info,downloaded,total}=>{
     p=p.child(format!("Downloading {} · {:.1} MiB",info.version,downloaded as f64/1048576.));
     if let Some(n)=total.filter(|n|*n>0){let f=(downloaded as f32/n as f32).clamp(0.,1.);p=p.child(div().h(px(10.)).bg(rgb(0x283a51)).child(div().w(relative(f)).h_full().bg(rgb(0x60a5fa))));}
    }
    UpdateStatus::Ready(info,_)=>p=p.child(format!("Version {} downloaded and verified",info.version)).child(button("install-studio","Restart and install").on_click(cx.listener(|v,_,_,cx|{
     if v.busy{return}match v.updater.apply_ready(){Ok(true)=>cx.quit(),Ok(false)=>{v.status="No staged update".into();cx.notify();},Err(e)=>{v.status=e;cx.notify();}}
    }))),
    UpdateStatus::Failed(e)=>p=p.child(div().text_color(rgb(0xfca5a5)).child(e)),
   }
   if !matches!(self.updater.status(),UpdateStatus::Checking|UpdateStatus::Downloading{..}|UpdateStatus::Ready(..)){
    p=p.child(button("check-studio","Check for updates").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.updater.start_check(false);v.update_poll(cx);cx.notify();}})));
   }
  }
  p.child("Private repository: unauthenticated shared updater cannot access private releases. No tokens are embedded. Download Studio previews through signed-in GitHub Releases. Studio beta tags use a separate namespace and are not supported by the existing latest-release updater; no automatic beta installation. Studio requires its own EXE and SHA256 asset, never the legacy executable.")
 }
 fn point_curves_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Bounded multipoint curves","Five monotone points per ink; endpoints locked."))
   .child("Applies a piecewise-linear device curve and resamples each B2A intent independently. Source caps and explicit TAC enforced. Initial support: simple ICC v4 CMYK / Lab mBA.");
  for i in 0..4{
   let mut row=div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x1b2637)).child(format!("Ink {}",i+1));
   for j in 1..4{
    row=row.child(div().flex().gap_1().items_center().child(format!("Input {}% → {:.0}%",j*25,self.curve_points[i][j]*100.))
     .child(button(("point-minus",i*5+j),"−5").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{let p=&mut v.curve_points[i];p[j]=(p[j]-0.05).max(p[j-1]);cx.notify();}})))
     .child(button(("point-plus",i*5+j),"+5").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{let p=&mut v.curve_points[i];p[j]=(p[j]+0.05).min(p[j+1]);cx.notify();}}))));
   }
   p=p.child(row);
  }
  p.child(button("reset-points","Reset multipoint curves").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.curve_points=IDENTITY_POINTS;cx.notify();}})))
   .child(self.correction_options(cx))
   .child(button("save-points","Generate bounded curves ICC").on_click(cx.listener(|v,_,_,cx|v.save_edit(7,cx))))
 }
 fn black_page(&self,cx:&mut Context<Self>)->Div{
  let s=self.black;
  let mut p=div().flex().flex_col().gap_3().child(heading("Black Generation / GCR / UCR","Constrained, predictive preference for physical channel 4."))
   .child("Verify channel 4 is actually black. GCR is a broad chroma-weighted preference; UCR favors dark near-neutrals. The solver may not reach the requested K curve. It preserves source channel maxima (K0 remains K0) and refuses a source over the selected TAC.")
   .child(div().flex().gap_2().child(button("gcr-mode",if !s.ucr{"GCR ✓"}else{"GCR"}).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.black.ucr=false;cx.notify();}})))
    .child(button("ucr-mode",if s.ucr{"UCR ✓"}else{"UCR"}).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.black.ucr=true;cx.notify();}}))));
  for (i,(name,value,step,lo,hi)) in [("Black start (darkness)",s.start,0.05,0.,0.95),("K target maximum",s.maximum,0.05,0.,1.),("Curve gamma",s.gamma,0.1,0.25,4.),("Preference strength",s.strength,0.05,0.,1.),("ΔE00 node tolerance",s.tolerance,0.1,0.1,5.)].into_iter().enumerate(){
   let label=if i==0||i==1||i==3{format!("{name}: {:.0}%",value*100.)}else{format!("{name}: {value:.2}")};
   let change=move|v:&mut Self,delta:f64|{let field=match i{0=>&mut v.black.start,1=>&mut v.black.maximum,2=>&mut v.black.gamma,3=>&mut v.black.strength,_=>&mut v.black.tolerance};*field=(*field+delta).clamp(lo,hi);};
   p=p.child(div().flex().items_center().gap_2().child(div().w(px(225.)).child(label))
    .child(button(("black-minus",i),"−").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{change(v,-step);cx.notify();}})))
    .child(button(("black-plus",i),"+").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{change(v,step);cx.notify();}}))));
  }
  p=p.child(div().text_sm().child("Requested K preference (before constraints, chroma and tolerance)"))
   .child(div().flex().items_end().h(px(70.)).children((0..=20).map(|i|div().flex_1().h(px((1.+68.*s.maximum*((i as f64/20.-s.start)/(1.-s.start)).clamp(0.,1.).powf(s.gamma)) as f32)).bg(rgb(0x9ca3af)))))
   .child(button("reset-black","Reset").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.black=BlackGeneration::default();cx.notify();}})));
  p.child(self.correction_options(cx))
   .child("Initial implementation: ICC v4 mBA, 17/33 grid. Per-node predicted appearance constraint, not a measured GCR/UCR calibration or a guarantee between nodes. Compare color, gradients and K distribution before production.")
   .child(button("save-black","Generate black preference ICC").on_click(cx.listener(|v,_,_,cx|v.save_edit(6,cx))))
 }
 fn proof_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Monitor Soft Proof","LittleCMS native proofing, tagged monitor-specific PNGs."))
   .child("Reference and candidate are printer profiles. If no candidate is selected, both proof images use the reference. The monitor must be calibrated; view exported PNGs in an external color-managed viewer.")
   .child(button("proof-source","Select image").on_click(cx.listener(|v,_,_,cx|{if !v.busy{if let Some(p)=rfd::FileDialog::new().add_filter("Image",&["png","jpg","jpeg","tif","tiff"]).pick_file(){v.proof_source=Some(p);v.proof_output=None;cx.notify();}}})))
   .child(self.proof_source.as_ref().map(|p|p.display().to_string()).unwrap_or("No image selected".into()))
   .child(button("proof-monitor","Select monitor ICC").on_click(cx.listener(|v,_,_,cx|{if !v.busy{if let Some(p)=rfd::FileDialog::new().add_filter("Display ICC",&["icc","icm"]).pick_file(){v.monitor_profile=Some(p);v.proof_output=None;cx.notify();}}})))
   .child(self.monitor_profile.as_ref().map(|p|p.display().to_string()).unwrap_or("No monitor profile selected".into()))
   .child(button("proof-media",if self.proof_media{"Simulate substrate white (absolute) ✓"}else{"Relative substrate simulation"}).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.proof_media=!v.proof_media;v.proof_output=None;cx.notify();}})))
   .child(button("proof-bpc",if self.proof_bpc{"Black point compensation ✓ (relative only)"}else{"Black point compensation off"}).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.proof_bpc=!v.proof_bpc;v.proof_output=None;cx.notify();}})))
   .child(format!("Separation intent: {:?} · untagged input: {} (Image Converter settings)",self.image_options.intent,if self.image_options.assume_untagged{"explicit assumption"}else{"reject"}))
   .child("This export samples a preview up to 640 pixels on the longest side; source image loading is limited to 24 MP. No change is made to the source ICC or image.")
   .child(button("export-monitor-proof","Export before / after monitor proof").on_click(cx.listener(|v,_,_,cx|{
    if v.busy{return}
    let (Some(source),Some(reference),Some(monitor))=(v.proof_source.clone(),v.source.clone(),v.monitor_profile.clone())else{v.status="Choose image, printer reference and monitor ICC.".into();cx.notify();return};
    let candidate=v.candidate.clone().unwrap_or_else(||reference.clone());
    let Some(root)=rfd::FileDialog::new().pick_folder()else{return};
    let options=images::ProofOptions{intent:v.image_options.intent,simulate_media:v.proof_media,bpc:v.proof_bpc,assume_untagged:v.image_options.assume_untagged};
    v.busy=true;v.proof_output=None;v.status="Rendering monitor proof…".into();cx.notify();
    cx.spawn(async move|this,cx|{
     let result=cx.background_spawn(async move{images::export_proof(&source,&reference,&candidate,&monitor,&root,options)}).await;
     let _=this.update(cx,|v,cx|{v.busy=false;match result{Ok(p)=>{v.status=format!("Proof exported: {}",p.display());v.proof_output=Some(p)},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
    }).detach();
   })));
  if let Some(path)=&self.proof_output{p=p.child(format!("Saved: {}",path.display())).child("before_monitor.png / after_monitor.png / proof_settings.txt");}
  p
 }
 fn preference_snapshot(&self)->Preferences{
  Preferences{schema:1,preview_open:self.preview_open,three_d:self.preview_options.three_d,wireframe:self.preview_options.wireframe,gamut_grid:self.gamut_grid,compare_grid:self.compare_grid,dpi:self.image_options.dpi,stream_enabled:self.stream_enabled,bigtiff:self.stream_options.bigtiff,separations:self.stream_options.separations,proof_media:self.proof_media,proof_bpc:self.proof_bpc,monitor:self.monitor_profile.clone(),library:self.library_root.clone(),output:self.image_root.clone()}
 }
 fn apply_preferences(&mut self,p:Preferences){
  self.preview_open=p.preview_open;self.preview_options.three_d=p.three_d;self.preview_options.wireframe=p.wireframe;
  self.gamut_grid=p.gamut_grid;self.compare_grid=p.compare_grid;self.image_options.dpi=p.dpi;
  self.stream_enabled=p.stream_enabled;self.stream_options.bigtiff=p.bigtiff;self.stream_options.separations=p.separations;
  self.proof_media=p.proof_media;self.proof_bpc=p.proof_bpc;self.monitor_profile=p.monitor.filter(|p|p.is_file());self.library_root=p.library.filter(|p|p.is_dir());self.image_root=p.output.filter(|p|p.is_dir());
  self.preview=None;self.comparison=None;self.compare_error=None;self.library_entries.clear();self.proof_output=None;
 }
 fn startup()->Self{
  let mut s=Self::default();
  match preferences::location().and_then(|p|preferences::load(&p)){
   Ok(p)=>s.apply_preferences(p),Err(e)=>s.status=format!("Preferences not loaded: {e}. Safe defaults are active.")
  }
  s
 }
 fn settings_page(&self,cx:&mut Context<Self>)->Div{
  div().flex().flex_col().gap_3().child(heading("Settings","Local preferences and color engine configuration."))
   .child("LittleCMS is linked statically. Official ArgyllCMS tools are installed by the portable backend when required.")
   .child(preferences::location().map(|p|p.display().to_string()).unwrap_or_else(|e|e))
   .child("Save preferences explicitly to restore preview mode, analysis grids, DPI, TIFF options, proof options, and last monitor/library/output paths next launch. No profile paths, TAC, ink caps, source assumptions or running jobs are restored automatically.")
   .child(button("prefs-save","Save current preferences as defaults").on_click(cx.listener(|v,_,_,cx|{
    if !v.busy{v.status=preferences::location().and_then(|p|preferences::save(&p,&v.preference_snapshot()).map(|_|format!("Preferences saved: {}",p.display()))).unwrap_or_else(|e|format!("Error: {e}"));cx.notify();}
   })))
   .child(button("prefs-reload","Reload saved preferences").on_click(cx.listener(|v,_,_,cx|{
    if !v.busy{match preferences::location().and_then(|p|preferences::load(&p)){Ok(p)=>{v.apply_preferences(p);v.status="Preferences loaded.".into();if v.preview_open{v.refresh_gamut(cx);}},Err(e)=>v.status=format!("Error: {e}")}cx.notify();}
   })))
   .child(button("prefs-reset","Reset preferences in memory (save to keep)").on_click(cx.listener(|v,_,_,cx|{
    if !v.busy{v.apply_preferences(Preferences::default());v.status="Default preferences restored in memory.".into();if v.preview_open{v.refresh_gamut(cx);}cx.notify();}
   })))
 }
 fn inspector(&self)->Div{
  let mut panel=div().flex().flex_col().gap_3().child(heading("Profile Inspector","ICC metadata decoded by LittleCMS 2."));
  if let Some(e)=&self.error{panel=panel.child(div().text_color(rgb(0xfca5a5)).child(e.clone()));}
  if let Some(r)=&self.report{
   panel=panel.child(format!("ICC v{:.2}  |  {}  |  {}",r.version,r.color_space,r.pcs))
    .child(format!("Class: {}  |  {:.2} MiB",r.device_class,r.size as f64/1048576.))
    .child(format!("Estimated total ink: {}",r.tac.map(|v|format!("{v:.1}%")).unwrap_or("Not available".into())))
    ;
   let mut white=div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x1b2637))
    .child(div().text_lg().child(if r.is_output{"Paper / Substrate Color"}else{"Media White Point"}))
    .child(div().text_xs().text_color(rgb(0x94a3b8)).child("ICC mediaWhitePointTag (wtpt), D50 PCS; not the header illuminant."));
   if let Some(w)=&r.media_white{
    let mut details=div().flex().flex_col().gap_1()
     .child(format!("XYZ (Y=1 reference): {:.5} / {:.5} / {:.5}",w.xyz[0],w.xyz[1],w.xyz[2]))
     .child(format!("Lab D50: L* {:.2}  a* {:.2}  b* {:.2}",w.lab[0],w.lab[1],w.lab[2]));
    if let Some([red,green,blue])=w.srgb{
     let hex=(u32::from(red)<<16)|(u32::from(green)<<8)|u32::from(blue);
     details=details.child(format!("Approx. sRGB #{hex:06X}"));
     white=white.child(div().flex().items_center().gap_3().child(div().w(px(64.)).h(px(64.)).rounded_md().bg(rgb(hex))).child(details));
    }else{white=white.child(details);}
    white=white.child(div().text_xs().child("Stored tag only. Swatch is an approximate sRGB rendering, not a calibrated or measured sample. A normalized D50 value does not prove the substrate is physically neutral."));
   }else{white=white.child("Not available: wtpt missing, unreadable or invalid. No substitute white is assumed.");}
   panel=panel.child(white)
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
   panel=panel.child("729 sRGB samples per intent. Sampled values are not exhaustive.");
   if let Some(bounds)=&r.strict_bounds{
    panel=panel.child(div().text_lg().child("Strict LUT bounds"));
    match bounds{
     Ok(b)=>{panel=panel.child(format!("Channel ceilings: {:.4?}%",b.channels.map(|v|v*100.))).child(format!("TAC upper bound: {:.4}% ({})",b.tac*100.,if b.exact_node_tac{"affine output curves; exact node maximum"}else{"conservative per-cell envelope; may overestimate"}));},
     Err(e)=>{panel=panel.child(format!("Not certifiable with supported LUT parser: {e}"));}
    }
   }
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
   Page::Correction=>self.correction_page(cx),
   Page::Smoothing=>self.smoothing_page(cx),
   Page::Converter=>self.image_page(cx),
   Page::DeviceLink=>self.link_page(cx),
   Page::Proof=>self.proof_page(cx),
   Page::Recipes=>self.recipe_page(cx),
   Page::Black=>self.black_page(cx),
   Page::Lab=>self.lab_page(cx),
   Page::Settings=>self.settings_page(cx),
   Page::About=>self.about_page(cx),
   Page::Library=>self.library_page(cx),
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
  cx.open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(bounds)),..Default::default()},|_,cx|cx.new(|_|Studio::startup())).expect("Open Studio window");
  cx.activate(true);
 });
}
