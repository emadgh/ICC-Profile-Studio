#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use gpui::{prelude::*, *};
use color_convert_rust::{Profile, backend::{self,Job,Mode}, inspection::{self,Inspection}};
use std::{path::PathBuf,sync::mpsc,time::Duration};
use color_convert_rust::comparison::{self,Comparison};
use color_convert_rust::editing::{self,Edit};
use color_convert_rust::correction::{Operation,Selective,BlackGeneration,IDENTITY_POINTS,MAX_REGIONS};
#[path = "../gamut_view.rs"] mod gamut_view;
use color_convert_rust::gamut::{self,Overlay};
use gamut_view::ViewOptions;
#[path = "../curve_view.rs"] mod curve_view;
use color_convert_rust::gamut_raster::CompareLayout;
use color_convert_rust::{device_link,versioning,project};
use color_convert_rust::gradient::{self,GradientReport};
use color_convert_rust::{library,reports,preferences::{self,Preferences}};
#[cfg(windows)] use update_via_github::{UpdateConfig,UpdateManager,UpdateStatus};
use color_convert_rust::{optimizer::{self,Settings as RecipeSettings,Report as RecipeReport},lab::{self,MeasuredComparison}};

const STUDIO_VERSION:&str=include_str!("../../STUDIO_VERSION");

#[derive(Clone,Copy,PartialEq,Eq)]
enum Page { Dashboard, Projects, Library, Inspector, Validator, Rebuilder, Comparison, Curves, Reorder, Metadata, Version, Gamut, Gradient, Correction, Smoothing, DeviceLink, Recipes, Black, Lab, Settings, About }
const PAGES: [(Page,&str);21] = [
 (Page::Dashboard,"Dashboard"),(Page::Projects,"Projects & Session"),(Page::Library,"Profile Library"),(Page::Inspector,"Profile Inspector"),(Page::Validator,"Profile Validator"),(Page::Rebuilder,"Profile Rebuilder"),
 (Page::Comparison,"Profile Comparison"),(Page::Curves,"Tone Curves"),(Page::Reorder,"Channel Reorder"),(Page::Metadata,"Profile Metadata"),(Page::Version,"ICC Version Conversion"),(Page::Gamut,"Gamut Viewer"),(Page::Gradient,"Gradient Analyzer"),(Page::Correction,"Color Correction"),(Page::Smoothing,"LUT Smoothing"),(Page::DeviceLink,"DeviceLink Generator"),(Page::Recipes,"Ink Optimizer"),(Page::Black,"Black Generation"),(Page::Lab,"Profiling Lab"),(Page::Settings,"Settings"),(Page::About,"About & Updates")
];
enum Event { Progress(String), Done(Result<String,String>) }
struct Studio {
 reference_identity:Option<project::FileIdentity>,project_path:Option<PathBuf>,version_allow_omit:bool,version_relative_only:bool,version_de:f64,version_ink:f64,version_report:Option<versioning::Report>,page:Page, source:Option<PathBuf>, profile:Option<Profile>, report:Option<Inspection>, error:Option<String>,
 caps:[f64;4],tac:f64,grid:usize,mode:Mode,status:String,busy:bool, candidate:Option<PathBuf>,comparison:Option<Comparison>,compare_error:Option<String>,compare_grid:usize,compare_intent:lcms2::Intent,gamma:[f64;4],order:[usize;4], preview_open:bool, preview:Option<Overlay>, preview_revision:u64, preview_width:f32,preview_fullscreen:bool,resize_last:Option<Pixels>,gamut_advanced:bool,curve_drag:Option<(usize,usize,Pixels)>, preview_cache:std::cell::RefCell<gamut_view::PreviewCache>, preview_options:ViewOptions, gamut_grid:usize,gradient:Option<GradientReport>,gradient_error:Option<String>,coverage_threshold:f64, regions:[Selective;MAX_REGIONS],region_count:usize,region_selected:usize, correction_grid:usize, correction_tac:f64, smoothing:f64, smoothing_passes:usize,orbit_last:Option<Point<Pixels>>,chart_dpi:u32,recipe_target:[f64;3],recipe_settings:RecipeSettings,recipe_result:Option<RecipeReport>,lab_levels:usize,measured:[Option<PathBuf>;2],measurement_result:Option<MeasuredComparison>,black:BlackGeneration,curve_points:[[f64;5];4],library_root:Option<PathBuf>,library_entries:Vec<library::Entry>,logo:std::sync::Arc<gpui::Image>,#[cfg(windows)] updater:UpdateManager,
}
impl Default for Studio {
 fn default()->Self{Self{reference_identity:None,project_path:None,version_allow_omit:false,version_relative_only:false,version_de:0.5,version_ink:0.5,version_report:None,page:Page::Dashboard,source:None,profile:None,report:None,error:None,caps:[100.,100.,100.,0.],tac:240.,grid:33,mode:Mode::Custom,status:"Open an ICC or ICM profile to start.".into(),busy:false,candidate:None,comparison:None,compare_error:None,compare_grid:17,compare_intent:lcms2::Intent::RelativeColorimetric,gamma:[1.;4],order:[0,1,2,3],preview_open:true,preview:None,preview_revision:0,preview_width:360.,preview_fullscreen:false,resize_last:None,gamut_advanced:false,curve_drag:None,preview_cache:std::cell::RefCell::new(gamut_view::PreviewCache::default()),preview_options:ViewOptions::default(),gamut_grid:17,gradient:None,gradient_error:None,coverage_threshold:2.,regions:[Selective::default();MAX_REGIONS],region_count:1,region_selected:0,correction_grid:33,correction_tac:400.,smoothing:0.20,smoothing_passes:1,orbit_last:None,chart_dpi:300,recipe_target:[50.,0.,0.],recipe_settings:RecipeSettings::default(),recipe_result:None,lab_levels:5,measured:[None,None],measurement_result:None,black:BlackGeneration::default(),curve_points:IDENTITY_POINTS,library_root:None,library_entries:vec![],logo:std::sync::Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png,include_bytes!("../../assets/icon.png").to_vec())),#[cfg(windows)] updater:UpdateManager::new(UpdateConfig::new("emadgh/color-convert-rust","ICC-Profile-Studio.exe",STUDIO_VERSION.trim()).with_app_name("ICC Profile Studio").with_checksum_asset("ICC-Profile-Studio.exe.sha256").with_required_checksum(true))}}
}
fn button(id: impl Into<ElementId>,label:impl Into<SharedString>)->Stateful<Div>{
 div().id(id).px_3().py_2().rounded_md().bg(rgb(0x263449)).text_color(rgb(0xe6eefb)).cursor_pointer().hover(|s|s.bg(rgb(0x354963))).child(label.into())
}
fn seg(id:impl Into<ElementId>,label:impl Into<SharedString>,active:bool)->Stateful<Div>{
 div().id(id).px_2().py_1().text_xs().rounded_md().bg(rgb(if active{0x38618a}else{0x263449})).text_color(rgb(0xe6eefb)).cursor_pointer().hover(|s|s.bg(rgb(0x46617c))).child(label.into())
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
  self.reference_identity=project::FileIdentity::capture(&path).ok();self.project_path=None;
  self.source=Some(path.clone());self.version_report=None;self.version_allow_omit=false;self.version_relative_only=false;self.black=BlackGeneration::default();self.curve_points=IDENTITY_POINTS;self.recipe_result=None;self.regions=[Selective::default();MAX_REGIONS];self.region_count=1;self.region_selected=0;self.correction_grid=33;self.correction_tac=400.;self.smoothing=0.20;self.smoothing_passes=1;self.preview=None;self.candidate=None;self.gradient=None;self.gradient_error=None;self.gamma=[1.;4];self.order=[0,1,2,3];self.comparison=None;self.compare_error=None;self.report=None;self.profile=None;self.error=None;
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
  if self.preview_open && self.source.is_some(){self.refresh_gamut(cx);}
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
  let edit=match kind{0=>Edit::Tone(self.gamma),1=>Edit::Reorder(self.order),3=>Edit::Correction(Operation::Regions{regions:self.regions,count:self.region_count},self.correction_grid,self.correction_tac),4=>Edit::Correction(Operation::Smooth{strength:self.smoothing,passes:self.smoothing_passes},self.correction_grid,self.correction_tac),5=>Edit::Correction(Operation::Optimize{weights:self.recipe_settings.weights,tolerance:self.recipe_settings.tolerance},self.correction_grid.min(33),self.correction_tac),6=>Edit::Correction(Operation::Black(self.black),self.correction_grid.min(33),self.correction_tac),7=>Edit::Correction(Operation::Points(self.curve_points),self.correction_grid,self.correction_tac),_=>Edit::Rename(destination.file_stem().unwrap_or_default().to_string_lossy().into_owned())};
  self.busy=true;self.status="Editing ICC and validating with LittleCMS…".into();cx.notify();
  cx.spawn(async move |this,cx|{
   let result=cx.background_spawn(async move{editing::edit_file(&source,&destination,edit)}).await;
   let _=this.update(cx,|view,cx|{view.busy=false;match result{Ok(s)=>{view.status=s;view.candidate=Some(output_path);view.preview=None;view.comparison=None;view.gradient=None;view.gradient_error=None;if view.preview_open{view.refresh_gamut(cx);}},Err(e)=>view.status=format!("Error: {e}")}cx.notify();});
  }).detach();
 }
 fn curve_cards(&self,point_mode:bool,cx:&mut Context<Self>)->Div{
  let names=self.profile.as_ref().map(|p|p.channels()).unwrap_or_default();
  let mut grid=div().flex().flex_col().gap_2();
  for row in 0..2{
   let mut line=div().flex().gap_2().w_full();
   for col in 0..2{
    let i=row*2+col;let name=names.get(i).cloned().unwrap_or_else(||format!("Ink {}",i+1));
    let color=[0x00b7e5,0xe33b91,0xe8c44d,0x9ca3af][i];
    let samples:Vec<f64>=(0..=256).map(|n|{let x=n as f64/256.;if point_mode{curve_view::linear(self.curve_points[i],x)}else{x.powf(self.gamma[i])}}).collect();
    let mut graph=curve_view::graph(&samples,color);
    for j in if point_mode{1..4}else{2..3}{
     let y=if point_mode{self.curve_points[i][j]}else{0.5_f64.powf(self.gamma[i])};
     graph=graph.child(curve_view::handle((if point_mode{"curve-point"}else{"curve-gamma"},i*5+j),j as f32/4.,y,color)
      .on_mouse_down(MouseButton::Left,cx.listener(move|v,e:&MouseDownEvent,_,cx|{if !v.busy{v.curve_drag=Some((i,if point_mode{j}else{5},e.position.y));cx.stop_propagation();}})));
    }
    let graph=graph.id((if point_mode{"point-graph"}else{"gamma-graph"},i))
     .on_mouse_move(cx.listener(move|v,e:&MouseMoveEvent,_,cx|{
      if !e.dragging(){v.curve_drag=None;return}
      if v.busy{return}
      if let Some((ink,j,last))=v.curve_drag{
       if ink!=i||point_mode!=(j<5){return}
       let delta=f32::from(last-e.position.y) as f64/curve_view::HEIGHT as f64;
       if j<5{let p=&mut v.curve_points[i];p[j]=(p[j]+delta).clamp(p[j-1],p[j+1]);}
       else{let y=(0.5_f64.powf(v.gamma[i])+delta).clamp(0.5_f64.powf(4.),0.5_f64.powf(0.25));v.gamma[i]=y.ln()/0.5_f64.ln();}
       v.curve_drag=Some((i,j,e.position.y));cx.notify();
      }
     }))
     .on_mouse_up(MouseButton::Left,cx.listener(|v,_,_,cx|{v.curve_drag=None;}));
    let mut card=div().flex().flex_col().flex_1().min_w(px(0.)).gap_1().p_2().rounded_md().bg(rgb(0x1b2637))
     .child(div().flex().items_center().justify_between().child(div().text_sm().child(name))
      .child(seg((if point_mode{"point-reset"}else{"gamma-reset"},i),"Reset",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{if point_mode{v.curve_points[i]=IDENTITY_POINTS[i];}else{v.gamma[i]=1.;}cx.notify();}}))))
     .child(graph)
     .child(div().flex().justify_between().text_xs().text_color(rgb(0x94a3b8)).child("0%").child("Input →").child("100%"));
    if point_mode{
     for j in 1..4{card=card.child(div().flex().gap_1().items_center().text_xs().child(format!("{}% → {:.0}%",j*25,self.curve_points[i][j]*100.))
      .child(seg(("point-minus",i*5+j),"−",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{let p=&mut v.curve_points[i];p[j]=(p[j]-0.05).max(p[j-1]);cx.notify();}})))
      .child(seg(("point-plus",i*5+j),"+",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{let p=&mut v.curve_points[i];p[j]=(p[j]+0.05).min(p[j+1]);cx.notify();}}))));}
    }else{
     card=card.child(div().flex().items_center().gap_1().child(seg(("gamma-minus",i),"−",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.gamma[i]=(v.gamma[i]-0.1).max(0.25);cx.notify();}}))).child(format!("γ {:.2}",self.gamma[i])).child(seg(("gamma-plus",i),"+",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.gamma[i]=(v.gamma[i]+0.1).min(4.);cx.notify();}}))))
      .child(div().text_xs().child(format!("25/50/75% → {:.0}/{:.0}/{:.0}%",100.*0.25_f64.powf(self.gamma[i]),100.*0.5_f64.powf(self.gamma[i]),100.*0.75_f64.powf(self.gamma[i]))));
    }
    line=line.child(card);
   }
   grid=grid.child(line);
  }
  grid
 }
 fn curves_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Tone Curves","Drag curve handles or use numeric steps. Curves represent corrections, not measured ink response."));
  let Some(profile)=&self.profile else{return p.child("Open a supported printer profile first.")};
  if !profile.cmyk(){return p.child("This editor supports four-channel CMYK printer profiles.")}
  p=p.child(self.curve_cards(false,cx))
   .child("Gamma modifies output separations; A2B stays unchanged. Recheck TAC and channel limits.")
   .child(button("save-curves","Save gamma ICC").on_click(cx.listener(|v,_,_,cx|v.save_edit(0,cx))))
   .child(self.point_curves_page(cx));
  p
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
 fn version_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("ICC Version Conversion","Structural conversion of CMYK / Lab printer profiles, verified in LittleCMS."));
  let Some(profile)=&self.profile else{return p.child("Open an ICC/ICM printer profile.")};
  if !profile.cmyk()||profile.pcs()!="Lab "||!matches!(profile.version_major(),2|4){return p.child("Only simple v2/v4 CMYK / Lab printer profiles are supported.")}
  let target=if profile.version_major()==2{4}else{2};
  let omitted=versioning::omitted_tags(profile);
  p=p.child(format!("ICC v{} → v{} · {}",profile.version_major(),target,if target==2{"lut16 (mft2)"}else{"mAB / mBA"}))
   .child("Preserves the six intent CLUTs and adapts the PCS-side Lab encoding; does not regenerate separations or relabel the header only. Unsupported curve/matrix structures fail closed.");
  if !omitted.is_empty(){
   p=p.child(div().text_sm().text_color(rgb(0xfbbf24)).child(format!("Optional / unhandled tags NOT transferred: {}",omitted.join(", "))))
    .child("These may include vendor metadata or gamut data. The original ICC remains intact. Only enable omission for an independent compatibility copy; vendor/RIP behavior is not guaranteed.")
    .child(seg("version-omit",if self.version_allow_omit{"Omit listed tags ✓"}else{"Allow omission of listed tags"},self.version_allow_omit).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.version_allow_omit=!v.version_allow_omit;cx.notify();}})));
  }
  for (i,(label,value)) in [("Max forward ΔE00",self.version_de),("Max channel Δ (percentage points)",self.version_ink)].into_iter().enumerate(){
   let change=move|v:&mut Self,d:f64|{let x=if i==0{&mut v.version_de}else{&mut v.version_ink};*x=(*x+d).clamp(0.05,2.);};
   p=p.child(div().flex().items_center().gap_2().child(div().w(px(220.)).child(label))
    .child(seg(("version-minus",i),"−",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{change(v,-0.05);cx.notify();}}))).child(format!("{value:.2}"))
    .child(seg(("version-plus",i),"+",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{change(v,0.05);cx.notify();}}))));
  }
  p=p.child(div().flex().gap_1().flex_wrap()
    .child(seg("version-all","Require all intents",!self.version_relative_only).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.version_relative_only=false;v.version_report=None;cx.notify();}})))
    .child(seg("version-rel","Relative / Absolute only",self.version_relative_only).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.version_relative_only=true;v.version_report=None;cx.notify();}}))))
   .when(self.version_relative_only,|p|p.child(div().text_sm().text_color(rgb(0xfbbf24)).child("Compatibility copy: Perceptual and Saturation may change substantially. Do not use those intents in the RIP. All four are measured and reported, but only Relative and Absolute must meet the selected tolerance.")))
   .child("Checks device→PCS color, PCS→device channel differences and TAC for all four intents. Preserves zero-ink channels and bounds; no new ICC saved if required limits fail.")
   .child(button("convert-icc-version",if self.busy{"Working…"}else{"Convert & validate new ICC"}).on_click(cx.listener(move|v,_,_,cx|{
    if v.busy{return}let Some(source)=v.source.clone()else{return};
    if !omitted.is_empty()&&!v.version_allow_omit{v.status="Review listed tags and explicitly allow omission, or keep the original ICC.".into();cx.notify();return}
    let name=format!("{}_v{target}{}.icc",source.file_stem().unwrap_or_default().to_string_lossy(),if v.version_relative_only{"_REL_ABS_ONLY"}else{""});
    let Some(mut dest)=rfd::FileDialog::new().add_filter("ICC",&["icc","icm"]).set_file_name(name).save_file()else{return};
    if dest.extension().is_none(){dest.set_extension("icc");}
    let (omit,de,ink,rel)=(v.version_allow_omit,v.version_de,v.version_ink,v.version_relative_only);
    v.version_report=None;v.busy=true;v.status="Converting LUT encoding and validating all intents…".into();cx.notify();
    cx.spawn(async move|this,cx|{
     let path=dest.clone();
     let result=cx.background_spawn(async move{versioning::convert_file(&source,&dest,target,omit,de,ink,rel)}).await;
     let _=this.update(cx,|v,cx|{v.busy=false;match result{
      Ok(r)=>{v.status=format!("Saved ICC v{} ({}): {}",r.target,if r.relative_absolute_only{"Relative/Absolute only"}else{"all intents validated"},path.display());v.version_report=Some(r);v.candidate=Some(path);v.preview=None;v.comparison=None;v.gradient=None;v.gradient_error=None;if v.preview_open{v.refresh_gamut(cx);}},
      Err(e)=>v.status=format!("Version conversion refused: {e}")
     }cx.notify();});
    }).detach();
   })));
  if let Some(r)=&self.version_report{
   p=p.child(div().text_lg().child("Validation report"))
    .child(format!("{} device + {} PCS samples per intent",r.forward_samples,r.reverse_samples))
    .child(format!("Channel bound {:?}% · TAC bound {:.3}%",r.bounds.channels.map(|x|100.*x),100.*r.bounds.tac));
   for i in &r.checks{p=p.child(div().text_sm().text_color(rgb(if i.passed{0xa7f3d0}else{0xfbbf24})).child(format!("{} [{}]: mean ΔE00 {:.4} · max {:.4} · channel Δ {:.4} pp · TAC Δ {:.4} pp",i.name,if i.passed{"PASS"}else if i.required{"FAIL"}else{"OUTSIDE TOLERANCE — NOT CERTIFIED"},i.mean_de,i.max_de,i.max_ink_pp,i.max_tac_pp)));}
   if !r.omitted.is_empty(){p=p.child(format!("Omitted by consent: {}",r.omitted.join(", ")));}
  }
  p.child("Sampled equivalence, not a guarantee for all colors or vendor compatibility. Always validate in the target RIP and print-test.")
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
 fn gamut_plot(&self,display:f32,id:&'static str,cx:&mut Context<Self>)->Stateful<Div>{
  let o=self.preview_options;
  div().id(id).w_full().flex_shrink_0().cursor_pointer()
    .on_mouse_down(MouseButton::Left,cx.listener(|this,e:&MouseDownEvent,_,cx|{this.orbit_last=Some(e.position);cx.notify();}))
    .on_mouse_up(MouseButton::Left,cx.listener(|this,_,_,cx|{this.orbit_last=None;cx.notify();}))
    .on_mouse_move(cx.listener(|this,e:&MouseMoveEvent,_,cx|{
     if !e.dragging(){this.orbit_last=None;return}
     if let Some(last)=this.orbit_last{
      if this.preview_options.three_d{
       let dx=f32::from(e.position.x-last.x);let dy=f32::from(e.position.y-last.y);
       if dx.abs()<100.&&dy.abs()<100.{
        this.preview_options.yaw=(this.preview_options.yaw-dx*0.01).rem_euclid(std::f32::consts::TAU);
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
    .child(gamut_view::plot(self.preview.as_ref(),self.preview_revision,o,&mut self.preview_cache.borrow_mut(),display))
 }
 fn gamut_toolbar(&self,cx:&mut Context<Self>)->Div{
  let o=self.preview_options;
  let style=|i:usize|o.style_label(i);
  let name=if self.candidate.is_some(){format!("Reference: {} · Candidate: {}",style(0),style(1))}else{format!("Reference only · {}",style(0))};
  div().flex().flex_col().gap_1().p_2().flex_shrink_0()
   .child(div().flex().items_center().gap_1()
    .child(seg("view-2d","2D",!o.three_d).on_click(cx.listener(|v,_,_,cx|{v.preview_options.three_d=false;cx.notify();})))
    .child(seg("view-3d","3D",o.three_d).on_click(cx.listener(|v,_,_,cx|{v.preview_options.three_d=true;cx.notify();})))
    .child(div().flex_1())
    .child(seg("ref-visible","Ref",o.reference).on_click(cx.listener(|v,_,_,cx|{v.preview_options.reference=!v.preview_options.reference;cx.notify();})))
    .when(self.candidate.is_some(),|p|p.child(seg("cand-visible","Cand",o.candidate).on_click(cx.listener(|v,_,_,cx|{v.preview_options.candidate=!v.preview_options.candidate;cx.notify();})))))
   .child(div().flex().gap_1().flex_wrap().children([("Pts",false,false),(if o.three_d{"Wire"}else{"Outline"},false,true),("Solid",true,false),("Both",true,true)].into_iter().enumerate().map(|(i,(name,solid,wire))|seg(("quick-style",i),name,o.layout==CompareLayout::Shared&&o.solid==solid&&o.wireframe==wire).on_click(cx.listener(move|v,_,_,cx|{v.preview_options.layout=CompareLayout::Shared;v.preview_options.solid=solid;v.preview_options.wireframe=wire;cx.notify();}))))
    .when(self.candidate.is_some(),|p|p.child(seg("mixed-quick","Compare",o.layout!=CompareLayout::Shared).on_click(cx.listener(|v,_,_,cx|{v.preview_options.layout=CompareLayout::ReferenceSolidCandidateWire;cx.notify();})))))
   .child(div().text_xs().text_color(rgb(0x94a3b8)).child(name))
   .when(!o.three_d,|p|p.child(div().flex().gap_1().items_center().flex_wrap()
    .child(seg("slice-toggle",if o.slice{"Slice"}else{"All L*"},o.slice).on_click(cx.listener(|v,_,_,cx|{v.preview_options.slice=!v.preview_options.slice;cx.notify();})))
    .when(o.slice,|p|p.child(seg("slice-down","−",false).on_click(cx.listener(|v,_,_,cx|{v.preview_options.light=(v.preview_options.light-5.).max(0.);cx.notify();}))).child(format!("L* {:.0}",o.light)).child(seg("slice-up","+",false).on_click(cx.listener(|v,_,_,cx|{v.preview_options.light=(v.preview_options.light+5.).min(100.);cx.notify();}))))
    .child(div().text_xs().child(format!("Fill {}%",o.opacity_2d)))
    .child(seg("opacity-down","−",false).on_click(cx.listener(|v,_,_,cx|{v.preview_options.opacity_2d=v.preview_options.opacity_2d.saturating_sub(10);cx.notify();})))
    .child(seg("opacity-up","+",false).on_click(cx.listener(|v,_,_,cx|{v.preview_options.opacity_2d=(v.preview_options.opacity_2d+10).min(100);cx.notify();})))))
 }
 fn preview_panel(&self,width:f32,display:f32,full:bool,cx:&mut Context<Self>)->Div{
  let panel=div().flex().flex_col().w(px(width)).flex_shrink_0().h_full().min_h(px(0.)).overflow_hidden().bg(rgb(0x101a2a))
   .child(div().flex().items_center().justify_between().p_2().flex_shrink_0()
    .child(div().text_sm().child("Gamut"))
    .child(div().flex().gap_1()
     .child(seg("full-gamut",if full{"Exit expanded"}else{"Expand"},full).on_click(cx.listener(|v,_,_,cx|{v.preview_fullscreen=!v.preview_fullscreen;cx.notify();})))
     .when(!full,|p|p.child(seg("collapse-preview","»",false).on_click(cx.listener(|v,_,_,cx|{v.preview_open=false;cx.notify();}))))))
   .child(self.gamut_toolbar(cx))
   .child(self.gamut_plot(display,"gamut-orbit",cx))
   .child(div().id("preview-scroll").flex_1().min_h(px(0.)).overflow_y_scroll().p_2().child(self.gamut_controls(cx)));
  panel
 }
 fn gamut_controls(&self,cx:&mut Context<Self>)->Div{
  let o=self.preview_options;
  let filename=|p:&Option<PathBuf>|p.as_ref().and_then(|p|p.file_name()).map(|n|n.to_string_lossy().into_owned()).unwrap_or("Not selected".into());
  let mut p=div().flex().flex_col().gap_2().text_sm()
   .child(div().text_xs().child(format!("Reference: {}",filename(&self.source))))
   .child(div().text_xs().child(format!("Candidate: {}",filename(&self.candidate))))
   .child(div().flex().gap_1().flex_wrap()
    .child(seg("preview-pick","Compare ICC",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy{if let Some(path)=rfd::FileDialog::new().add_filter("ICC profile",&["icc","icm"]).pick_file(){v.candidate=Some(path);v.preview_options.candidate=true;v.preview=None;v.comparison=None;v.gradient=None;v.gradient_error=None;v.refresh_gamut(cx);cx.notify();}}})))
    .when(self.candidate.is_some(),|p|p.child(seg("clear-candidate","Clear",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.candidate=None;v.preview=None;v.comparison=None;v.gradient=None;v.gradient_error=None;v.refresh_gamut(cx);cx.notify();}}))))
    .child(seg("refresh-gamut",if self.busy{"Working…"}else{"Refresh"},false).on_click(cx.listener(|v,_,_,cx|v.refresh_gamut(cx)))))
   .child(div().flex().gap_1().items_center().child(div().text_xs().child("Quality")).children([9usize,17,33].map(|n|seg(("gamut-grid",n),match n{9=>"Draft",17=>"Normal",_=>"High"},self.gamut_grid==n).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.gamut_grid=n;v.refresh_gamut(cx);cx.notify();}})))))
   .child(div().flex().gap_1().items_center().child(seg("zoom-out","−",false).on_click(cx.listener(|v,_,_,cx|{v.preview_options.zoom=(v.preview_options.zoom/1.2).max(0.5);cx.notify();}))).child(format!("{:.0}%",100.*o.zoom)).child(seg("zoom-in","+",false).on_click(cx.listener(|v,_,_,cx|{v.preview_options.zoom=(v.preview_options.zoom*1.2).min(3.);cx.notify();}))).child(seg("reset-view","Reset",false).on_click(cx.listener(|v,_,_,cx|{v.preview_options=ViewOptions::default();cx.notify();}))))
   .child(seg("advanced-gamut",if self.gamut_advanced{"Advanced ▴"}else{"Advanced ▾"},self.gamut_advanced).on_click(cx.listener(|v,_,_,cx|{v.gamut_advanced=!v.gamut_advanced;cx.notify();})));
  if let Some(d)=&self.preview{
   if let Err(e)=&d.reference{p=p.child(format!("Reference: {e}"));}
   if let Some(Err(e))=&d.candidate{p=p.child(format!("Candidate: {e}"));}
   let n=usize::from(d.reference.is_ok()&&o.reference)+usize::from(d.candidate.as_ref().is_some_and(|c|c.is_ok())&&o.candidate);
   p=p.child(div().text_xs().text_color(rgb(0x94a3b8)).child(format!("{n} active profile(s). Floor = one footprint per profile; disconnected regions may have separate outlines.")));
  }
  if self.gamut_advanced{
   p=p.child(div().flex().gap_1().flex_wrap()
    .child(seg("ref-solid","Ref solid",o.layout==CompareLayout::ReferenceSolidCandidateWire).on_click(cx.listener(|v,_,_,cx|{v.preview_options.layout=CompareLayout::ReferenceSolidCandidateWire;cx.notify();})))
    .child(seg("cand-solid","Cand solid",o.layout==CompareLayout::ReferenceWireCandidateSolid).on_click(cx.listener(|v,_,_,cx|{v.preview_options.layout=CompareLayout::ReferenceWireCandidateSolid;cx.notify();}))))
    .child(div().flex().gap_1().flex_wrap()
     .child(seg("profile-colors","Profile colors",o.profile_colors).on_click(cx.listener(|v,_,_,cx|{v.preview_options.profile_colors=true;cx.notify();})))
     .child(seg("compare-tint","Compare tint",!o.profile_colors).on_click(cx.listener(|v,_,_,cx|{v.preview_options.profile_colors=false;cx.notify();}))))
    .child(div().flex().gap_1().flex_wrap()
     .child(seg("surface","Surface",!o.convex).on_click(cx.listener(|v,_,_,cx|{v.preview_options.convex=false;cx.notify();})))
     .child(seg("convex","Convex",o.convex).on_click(cx.listener(|v,_,_,cx|{v.preview_options.convex=true;cx.notify();})))
     .child(seg("floor","Floor",o.floor_contour).on_click(cx.listener(|v,_,_,cx|{v.preview_options.floor_contour=!v.preview_options.floor_contour;cx.notify();}))))
    .child(seg("save-gamut-png","Export PNG",false).on_click(cx.listener(|v,_,_,cx|{if let Some(data)=&v.preview{if let Some(path)=rfd::FileDialog::new().add_filter("PNG",&["png"]).set_file_name("gamut.png").save_file(){let result=color_convert_rust::gamut_raster::render(Some(data),v.preview_options).png().and_then(|bytes|{use std::io::Write;let mut f=tempfile::NamedTempFile::new_in(path.parent().unwrap_or(std::path::Path::new("."))).map_err(|e|e.to_string())?;f.write_all(&bytes).map_err(|e|e.to_string())?;f.persist_noclobber(&path).map_err(|e|e.to_string())?;Ok(())});v.status=result.map(|_|format!("Saved {}",path.display())).unwrap_or_else(|e|format!("Error: {e}"));cx.notify();}}})))
    .child(div().text_xs().text_color(rgb(0x94a3b8)).child("Sampled round-trip ΔE00 ≤ 2 surface; profile colors approximate D50 Lab to sRGB. Wire grid = L* rings and hue meridians. Not a measured physical gamut."));
  }
  p
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
   .child(self.gamut_plot(320.,"gamut-page-orbit",cx))
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
  let mut p=div().flex().flex_col().gap_3().child(heading("Selective Color Correction","Up to six independent soft hue regions in D50 Lab/LCh."));
  let Some(profile)=&self.profile else{return p.child("Open a compatible ICC v4 CMYK printer profile.")};
  if !profile.cmyk()||profile.version_major()!=4||profile.pcs()!="Lab "{return p.child("Supports ICC v4 CMYK / Lab with simple mBA LUTs only.")}
  let selected=self.region_selected;let r=self.regions[selected];
  p=p.child(div().flex().gap_1().flex_wrap().items_center().children((0..self.region_count).map(|i|{
   let r=self.regions[i];let rgb=color_convert_rust::gamut_raster::lab_rgb([65.,(45.*r.center.to_radians().cos()) as f32,(45.*r.center.to_radians().sin()) as f32]);
   let color=(u32::from(rgb[0])<<16)|(u32::from(rgb[1])<<8)|u32::from(rgb[2]);
   seg(("region",i),format!("{} · {:.0}°{}",i+1,r.center,if r.active(){" *"}else{""}),i==selected)
    .child(div().size(px(8.)).rounded_md().bg(gpui::rgb(color)))
    .on_click(cx.listener(move|v,_,_,cx|{v.region_selected=i;cx.notify();}))
  }))
   .when(self.region_count<MAX_REGIONS,|p|p.child(seg("add-region","+ Region",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy&&v.region_count<MAX_REGIONS{v.regions[v.region_count]=Selective{center:(v.regions[v.region_selected].center+60.).rem_euclid(360.),..Default::default()};v.region_selected=v.region_count;v.region_count+=1;cx.notify();}}))))
   .when(self.region_count>1,|p|p.child(seg("remove-region","Remove",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy&&v.region_count>1{let i=v.region_selected;v.regions.copy_within(i+1..v.region_count,i);v.region_count-=1;v.regions[v.region_count]=Selective::default();v.region_selected=i.min(v.region_count-1);cx.notify();}})))));
  p=p.child(div().flex().gap_1().flex_wrap().children([("Red",40.,0xc34c4c),("Yellow",90.,0xdbbb35),("Green",135.,0x47a86a),("Cyan",200.,0x2fa6b8),("Blue",270.,0x547cda),("Magenta",320.,0xb865b5)].into_iter().enumerate().map(|(i,(name,hue,color))|seg(("hue-preset",i),name,false).bg(rgb(color)).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.regions[v.region_selected].center=hue;cx.notify();}})))));
  for (i,(name,value,lo,hi,step,unit)) in [("Hue center",r.center,0.,360.,5.,"°"),("Hue half-width",r.width,10.,180.,5.,"°"),("Lightness",r.light,-30.,30.,1.," L*"),("Chroma",r.chroma_percent,-80.,80.,5.,"%"),("Hue shift",r.hue_shift,-60.,60.,5.,"°")].into_iter().enumerate(){
   let change=move|v:&mut Self,delta:f64|{let r=&mut v.regions[v.region_selected];let f=match i{0=>&mut r.center,1=>&mut r.width,2=>&mut r.light,3=>&mut r.chroma_percent,_=>&mut r.hue_shift};*f=(*f+delta).clamp(lo,hi);};
   p=p.child(div().flex().items_center().gap_2().child(div().w(px(120.)).child(name))
    .child(seg(("correction-minus",i),"−",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{change(v,-step);cx.notify();}})))
    .child(div().w(px(75.)).child(format!("{value:.0}{unit}")))
    .child(seg(("correction-plus",i),"+",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{change(v,step);cx.notify();}}))));
  }
  let active=self.regions[..self.region_count].iter().filter(|r|r.active()).count();
  p.child(div().flex().gap_1().child(seg("reset-selective","Reset region",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.regions[v.region_selected]=Selective::default();cx.notify();}})))
   .child(seg("reset-regions","Reset all",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.regions=[Selective::default();MAX_REGIONS];v.region_count=1;v.region_selected=0;cx.notify();}}))))
   .child(format!("{active} active correction region(s). Overlap is normalized; all masks use the original PCS color. Near-neutrals and lightness endpoints remain protected."))
   .child(self.correction_options(cx))
   .child("One resampling pass for all regions, independent B2A intents; preserves A2B. Resampling can change unselected colors. Compare the new ICC before printing.")
   .child(button("save-selective","Generate corrected ICC").on_click(cx.listener(|v,_,_,cx|{if v.regions[..v.region_count].iter().any(|r|r.active()){v.save_edit(3,cx);}else{v.status="Choose a nonzero region adjustment first.".into();cx.notify();}})))
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
 fn link_page(&self,cx:&mut Context<Self>)->Div{
  div().flex().flex_col().gap_3().child(heading("DeviceLink Generator","LittleCMS CMYK → CMYK ICC v4.3."))
   .child("Reference is source CMYK and candidate is destination. Select the candidate in Gamut Preview or Profile Comparison. Relative colorimetric; no special black preservation or certified channel cap guarantee.")
   .child(button("save-link","Generate DeviceLink").on_click(cx.listener(|v,_,_,cx|{
    if v.busy{return}let (Some(a),Some(b))=(v.source.clone(),v.candidate.clone())else{v.status="Select two printer profiles.".into();cx.notify();return};
    let Some(mut dest)=rfd::FileDialog::new().add_filter("ICC",&["icc"]).set_file_name("CMYK_to_CMYK.icc").save_file()else{return};if dest.extension().is_none(){dest.set_extension("icc");}
    v.busy=true;v.status="Generating DeviceLink…".into();cx.notify();
    cx.spawn(async move|this,cx|{
     let result=cx.background_spawn(async move{device_link::device_link(&a,&b,&dest,lcms2::Intent::RelativeColorimetric).map(|r|format!("Saved {} | {} samples | Max link/direct channel deviation {:.3} pp | Peak sampled TAC {:.2}% | Max channels {:.2?}%",dest.display(),r.sample_count,r.max_channel_difference,r.peak_tac,r.maxima))}).await;
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
  let mut p=div().flex().flex_col().gap_3().child(heading("Profiling Lab","Direct-ink test charts, measured patch comparison and ArgyllCMS profiling."))
   .child("Record printer, glaze, ink batch, firing schedule and measurement conditions externally. Use identical measurement geometry and D50 Lab for both charts.")
   .child(div().text_lg().child("Generate device test chart"))
   .child(div().flex().gap_1().items_center().child("Levels per ink").children([3usize,5,7].map(|n|button(("chart-level",n),if self.lab_levels==n{format!("{n} ✓")}else{n.to_string()}).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.lab_levels=n;cx.notify();}})))))
   .child(format!("Rebuilder caps {:?}, TAC {:.0}%; Chart DPI {}. 8 mm patches, 2 mm gaps.",self.caps,self.tac,self.chart_dpi))
   .child(div().flex().gap_2().items_center().child(format!("Chart resolution: {} DPI",self.chart_dpi))
    .child(button("chart-dpi-minus","−10").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.chart_dpi=v.chart_dpi.saturating_sub(10).max(50);cx.notify();}})))
    .child(button("chart-dpi-plus","+10").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.chart_dpi=(v.chart_dpi+10).min(1200);cx.notify();}}))))
   .child(button("generate-chart","Export TIFF chart and measurement CSV").on_click(cx.listener(|v,_,_,cx|{
    if v.busy{return}let Some(p)=v.source.clone()else{return};let Some(root)=rfd::FileDialog::new().pick_folder()else{return};
    let (levels,caps,tac,dpi)=(v.lab_levels,v.caps,v.tac,v.chart_dpi);
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
  p.child("The shared updater cannot access private releases without authentication. No tokens are embedded. Download Studio previews through signed-in GitHub Releases. Studio beta tags use a separate namespace and are not supported by the existing latest-release updater; no automatic beta installation. Studio requires its own EXE and SHA256 asset, never the legacy executable.")
 }
 fn point_curves_page(&self,cx:&mut Context<Self>)->Div{
  div().flex().flex_col().gap_2().child(heading("Bounded multipoint curves","Drag three control points per ink. Endpoints locked; monotonic output enforced."))
   .child(self.curve_cards(true,cx))
   .child("Piecewise-linear device correction; independent B2A intents, source caps and explicit TAC. Supported simple v4 CMYK/Lab mBA.")
   .child(seg("reset-points","Reset all points",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.curve_points=IDENTITY_POINTS;cx.notify();}})))
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
  let samples:Vec<f64>=(0..=256).map(|i|s.maximum*((i as f64/256.-s.start)/(1.-s.start)).clamp(0.,1.).powf(s.gamma)).collect();
  let mid=s.start+(1.-s.start)*0.5;
  let graph=curve_view::graph(&samples,0xcbd5e1)
   .child(curve_view::handle("black-mid",mid as f32,s.maximum*0.5_f64.powf(s.gamma),0xcbd5e1)
    .on_mouse_down(MouseButton::Left,cx.listener(|v,e:&MouseDownEvent,_,cx|{if !v.busy&&v.black.maximum>0.01{v.curve_drag=Some((4,5,e.position.y));cx.stop_propagation();}})))
   .child(curve_view::handle("black-max",0.97,s.maximum,0xcbd5e1)
    .on_mouse_down(MouseButton::Left,cx.listener(|v,e:&MouseDownEvent,_,cx|{if !v.busy{v.curve_drag=Some((4,4,e.position.y));cx.stop_propagation();}})))
   .id("black-curve-graph")
   .on_mouse_move(cx.listener(|v,e:&MouseMoveEvent,_,cx|{
    if !e.dragging(){v.curve_drag=None;return}
    if v.busy{return}
    if let Some((4,j,last))=v.curve_drag{
     let delta=f32::from(last-e.position.y) as f64/curve_view::HEIGHT as f64;
     if j==4{v.black.maximum=(v.black.maximum+delta).clamp(0.,1.);}
     else if v.black.maximum>0.01{let y=(0.5_f64.powf(v.black.gamma)+delta/v.black.maximum).clamp(0.5_f64.powf(4.),0.5_f64.powf(0.25));v.black.gamma=y.ln()/0.5_f64.ln();}
     v.curve_drag=Some((4,j,e.position.y));cx.notify();
    }
   }))
   .on_mouse_up(MouseButton::Left,cx.listener(|v,_,_,cx|{v.curve_drag=None;}));
  p=p.child(div().text_sm().child("Requested K preference (before constraints, chroma and tolerance)"))
   .child(graph).child(div().flex().justify_between().text_xs().child("0% darkness").child("100% darkness")).child(div().text_xs().child("Drag midpoint for gamma or right handle for maximum K. The solver remains constrained; requested curve is not guaranteed."))
   .child(button("reset-black","Reset").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.black=BlackGeneration::default();cx.notify();}})));
  p.child(self.correction_options(cx))
   .child("Initial implementation: ICC v4 mBA, 17/33 grid. Per-node predicted appearance constraint, not a measured GCR/UCR calibration or a guarantee between nodes. Compare color, gradients and K distribution before production.")
   .child(button("save-black","Generate black preference ICC").on_click(cx.listener(|v,_,_,cx|v.save_edit(6,cx))))
 }
 fn project_snapshot(&self)->Result<project::Project,String>{
  let reference=self.reference_identity.clone().ok_or("Open a valid reference ICC first.")?;
  if self.report.is_none(){return Err("Reference ICC did not pass inspection".into())}
  reference.verify()?;
  let candidate=self.candidate.as_ref().map(|p|project::FileIdentity::capture(p)).transpose()?;
  let compare_intent=match self.compare_intent{lcms2::Intent::Perceptual=>0,lcms2::Intent::RelativeColorimetric=>1,lcms2::Intent::Saturation=>2,lcms2::Intent::AbsoluteColorimetric=>3,_=>1};
  Ok(project::Project{schema:1,reference,candidate,settings:project::Settings{
   caps:self.caps,tac:self.tac,grid:self.grid,argyll:self.mode==Mode::Argyll,gamma:self.gamma,order:self.order,curve_points:self.curve_points,
   regions:self.regions,region_count:self.region_count,region_selected:self.region_selected,correction_grid:self.correction_grid,correction_tac:self.correction_tac,smoothing:self.smoothing,smoothing_passes:self.smoothing_passes,
   black:self.black,recipe_target:self.recipe_target,recipe_weights:self.recipe_settings.weights,recipe_tolerance:self.recipe_settings.tolerance,
   gamut:self.preview_options,gamut_grid:self.gamut_grid,compare_grid:self.compare_grid,compare_intent,coverage_threshold:self.coverage_threshold,preview_open:self.preview_open,preview_width:self.preview_width,chart_dpi:self.chart_dpi,lab_levels:self.lab_levels}})
 }
 fn restore_project(&mut self,path:PathBuf,cx:&mut Context<Self>)->Result<(),String>{
  let p=project::load(&path)?;
  // Validate all references and native ICC parsing before changing the current workspace.
  inspection::inspect(&p.reference.path)?;
  if let Some(c)=&p.candidate{inspection::inspect(&c.path)?;}
  p.validate()?;
  self.preview_open=false;self.load_reference(p.reference.path.clone(),cx);
  self.reference_identity=Some(p.reference);self.candidate=p.candidate.map(|c|c.path);
  let x=p.settings;
  self.caps=x.caps;self.tac=x.tac;self.grid=x.grid;self.mode=if x.argyll{Mode::Argyll}else{Mode::Custom};
  self.gamma=x.gamma;self.order=x.order;self.curve_points=x.curve_points;self.regions=x.regions;self.region_count=x.region_count;self.region_selected=x.region_selected;
  self.correction_grid=x.correction_grid;self.correction_tac=x.correction_tac;self.smoothing=x.smoothing;self.smoothing_passes=x.smoothing_passes;self.black=x.black;
  self.recipe_target=x.recipe_target;self.recipe_settings.weights=x.recipe_weights;self.recipe_settings.tolerance=x.recipe_tolerance;
  self.preview_options=x.gamut;self.gamut_grid=x.gamut_grid;self.compare_grid=x.compare_grid;self.compare_intent=[lcms2::Intent::Perceptual,lcms2::Intent::RelativeColorimetric,lcms2::Intent::Saturation,lcms2::Intent::AbsoluteColorimetric][x.compare_intent as usize];self.coverage_threshold=x.coverage_threshold;
  self.preview_open=x.preview_open;self.preview_width=x.preview_width;self.chart_dpi=x.chart_dpi;self.lab_levels=x.lab_levels;self.preview_fullscreen=false;self.measured=[None,None];self.measurement_result=None;
  self.project_path=Some(path);self.page=Page::Projects;self.status="Project restored. Verify physical channel mapping and limits before generating a new ICC.".into();
  // Leave computation explicitly opt-in; never restart saved jobs.
  cx.notify();Ok(())
 }
 fn projects_page(&self,cx:&mut Context<Self>)->Div{
  div().flex().flex_col().gap_3().child(heading("Projects & Session","Explicit ICC workspace snapshots with SHA-256 profile identity."))
   .child(format!("Project: {}",self.project_path.as_ref().map(|p|p.display().to_string()).unwrap_or("Unsaved".into())))
   .child("Stores reference/candidate file paths, ink limits, curves, color correction regions, black settings, optimization and gamut settings. Does not embed ICCs, measurement files or computed reports. Keep external ICC files unchanged at their original paths.")
   .child(div().flex().gap_2().flex_wrap()
    .child(button("project-save","Save project as…").on_click(cx.listener(|v,_,_,cx|{
     if v.busy{return}let Some(mut path)=rfd::FileDialog::new().add_filter("ICC Project",&["iccproj"]).set_file_name("workspace.iccproj").save_file()else{return};if path.extension().is_none(){path.set_extension("iccproj");}
     match v.project_snapshot().and_then(|p|project::save(&path,&p,false)){Ok(())=>{v.project_path=Some(path);v.status="Project saved.".into();},Err(e)=>v.status=format!("Project save refused: {e}")}cx.notify();
    })))
    .child(button("project-open","Open project…").on_click(cx.listener(|v,_,_,cx|{
     if v.busy{return}let Some(path)=rfd::FileDialog::new().add_filter("ICC Project",&["iccproj"]).pick_file()else{return};
     if let Err(e)=v.restore_project(path,cx){v.status=format!("Project open refused: {e}");}cx.notify();
    }))))
   .child(div().text_lg().child("Session checkpoint"))
   .child("Optional explicit recovery snapshot. Nothing is automatically restored on startup. A checkpoint replaces only the previous app-owned session checkpoint.")
   .child(div().flex().gap_2().flex_wrap()
    .child(button("checkpoint-save","Save checkpoint").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.status=project::checkpoint().and_then(|path|v.project_snapshot().and_then(|p|project::save(&path,&p,true))).map(|_|"Session checkpoint saved.".into()).unwrap_or_else(|e|format!("Checkpoint refused: {e}"));cx.notify();}})))
    .child(button("checkpoint-open","Restore checkpoint").on_click(cx.listener(|v,_,_,cx|{if !v.busy{if let Err(e)=project::checkpoint().and_then(|path|v.restore_project(path,cx)){v.status=format!("Recovery refused: {e}");}cx.notify();}}))))
   .child("Save as a new project file to keep revisions. Missing or modified profiles cause restoration to be refused. Recompute gamut and comparisons manually after opening.")
 }
 fn preference_snapshot(&self)->Preferences{
  Preferences{schema:1,preview_open:self.preview_open,three_d:self.preview_options.three_d,wireframe:self.preview_options.wireframe,solid:self.preview_options.solid,profile_colors:self.preview_options.profile_colors,convex:self.preview_options.convex,layout:self.preview_options.layout,floor_contour:self.preview_options.floor_contour,preview_width:self.preview_width,opacity_2d:self.preview_options.opacity_2d,gamut_grid:self.gamut_grid,compare_grid:self.compare_grid,chart_dpi:self.chart_dpi,library:self.library_root.clone()}
 }
 fn apply_preferences(&mut self,p:Preferences){
  self.preview_open=p.preview_open;self.preview_options.three_d=p.three_d;self.preview_options.wireframe=p.wireframe;self.preview_options.solid=p.solid;self.preview_options.profile_colors=p.profile_colors;self.preview_options.convex=p.convex;self.preview_options.layout=p.layout;self.preview_options.floor_contour=p.floor_contour;self.preview_width=p.preview_width;self.preview_options.opacity_2d=p.opacity_2d;
  self.gamut_grid=p.gamut_grid;self.compare_grid=p.compare_grid;self.chart_dpi=p.chart_dpi;
  self.library_root=p.library.filter(|p|p.is_dir());
  self.preview=None;self.comparison=None;self.compare_error=None;self.library_entries.clear();
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
   .child("Save preferences explicitly to restore gamut preview mode, analysis grids, characterization chart DPI and last library folder next launch. No profile paths, TAC, ink caps or running jobs are restored automatically.")
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
 fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement{
  let viewport=window.viewport_size();let vw=f32::from(viewport.width);let vh=f32::from(viewport.height);
  let width=self.preview_width.clamp(280.,(vw-460.).max(280.));
  let display=(width-24.).min((vh-240.).max(180.));
  if self.preview_fullscreen{return div().flex().size_full().bg(rgb(0x101a2a)).text_color(rgb(0xe2e8f0)).child(self.preview_panel(vw,(vw-28.).min((vh-220.).max(180.)),true,cx));}
  let sidebar=div().id("sidebar").flex().flex_col().w(px(224.)).flex_shrink_0().h_full().overflow_y_scroll().p_3().gap_1().bg(rgb(0x101a2a))
   .child(div().text_lg().child("ICC Profile Studio")).child(div().text_xs().mb_3().child(format!("{} · Gamut R5",STUDIO_VERSION.trim())))
   .child(button("open-profile","Open ICC / ICM").on_click(cx.listener(|this,_,_,cx|{this.open(cx);cx.notify();})))
   .children(PAGES.into_iter().enumerate().map(|(i,(page,label))|{
    let active=self.page==page;
    div().id(("page",i)).px_3().py_2().rounded_md().text_sm().cursor_pointer()
     .bg(rgb(if active{0x304867}else{0x101a2a})).hover(|s|s.bg(rgb(0x233751))).child(label)
     .on_click(cx.listener(move|this,_,_,cx|{this.page=page;if page==Page::Gamut&&this.preview.is_none(){this.refresh_gamut(cx);}cx.notify();}))
   }));
  let content=match self.page{
   Page::Dashboard=>div().flex().flex_col().gap_3().child(heading("Dashboard","ICC profile inspection, editing and characterization.")).child("Open an ICC/ICM profile to inspect, edit, validate, compare or rebuild it. Image conversion and editing are outside the current Studio scope."),
   Page::Projects=>self.projects_page(cx),
   Page::Inspector=>self.inspector(),
   Page::Validator=>self.validator(),
   Page::Rebuilder=>self.rebuilder(cx),
   Page::Comparison=>self.comparison_page(cx),
   Page::Curves=>self.curves_page(cx),
   Page::Reorder=>self.reorder_page(cx),
   Page::Metadata=>self.metadata_page(cx),
   Page::Version=>self.version_page(cx),
   Page::Gamut=>self.gamut_analysis_page(cx),
   Page::Gradient=>self.gradient_page(cx),
   Page::Correction=>self.correction_page(cx),
   Page::Smoothing=>self.smoothing_page(cx),
   Page::DeviceLink=>self.link_page(cx),
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
   .when(self.preview_open,|root|root.child(div().id("gamut-resizer").w(px(6.)).h_full().flex_shrink_0().bg(rgb(0x24364b)).hover(|s|s.bg(rgb(0x60a5fa))).cursor_pointer()
    .on_mouse_down(MouseButton::Left,cx.listener(|v,e:&MouseDownEvent,_,cx|{v.resize_last=Some(e.position.x);cx.stop_propagation();}))
    .on_mouse_up(MouseButton::Left,cx.listener(|v,_,_,cx|{v.resize_last=None;cx.notify();})))
    .child(self.preview_panel(width,display,false,cx)))
   .on_mouse_move(cx.listener(move|v,e:&MouseMoveEvent,_,cx|{
    if !e.dragging(){v.resize_last=None;return}
    if let Some(last)=v.resize_last{v.preview_width=(v.preview_width-f32::from(e.position.x-last)).clamp(280.,(vw-460.).max(280.));v.resize_last=Some(e.position.x);cx.notify();}
   }))
   .on_mouse_up(MouseButton::Left,cx.listener(|v,_,_,cx|{v.resize_last=None;v.curve_drag=None;}))
 }
}
fn main(){
 Application::new().run(|cx:&mut App|{
  let bounds=Bounds::centered(None,size(px(1440.),px(880.)),cx);
  cx.open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(bounds)),..Default::default()},|_,cx|cx.new(|_|Studio::startup())).expect("Open Studio window");
  cx.activate(true);
 });
}
