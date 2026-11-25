#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use gpui::{prelude::*, *};
use color_convert_rust::capabilities::{self,Tool,Capabilities};
use color_convert_rust::{ui_geometry::{self,Scale},Profile, backend::{self,Job,Mode}, inspection::{self,Inspection}};
use std::{path::PathBuf,sync::mpsc,time::Duration};
use color_convert_rust::comparison::{self,Comparison};
use color_convert_rust::editing::{self,Edit};
use color_convert_rust::correction::{DynamicCorrection,Selective,BlackGeneration,IDENTITY_POINTS,MAX_REGIONS};
#[path = "../gamut_view.rs"] mod gamut_view;
use color_convert_rust::gamut::{self,Overlay};
use gamut_view::ViewOptions;
#[path = "../curve_view.rs"] mod curve_view;
use color_convert_rust::gamut_raster::CompareLayout;
use color_convert_rust::{cie_spectral::{self,Illuminant,Observer},characterization_tools as target_tools,device_link,versioning,system_profiles,i18n::{self,Language,tr}};
use color_convert_rust::gradient::{self,GradientReport};
use color_convert_rust::{reports,preferences::{self,Preferences}};
#[cfg(windows)] use update_via_github::{UpdateConfig,UpdateStatus};
#[cfg(windows)] #[path="../studio_updates.rs"] mod studio_updates;
#[cfg(windows)] use studio_updates::StudioUpdater;
use color_convert_rust::{optimizer::{self,MultiSettings as RecipeSettings,MultiReport as RecipeReport},lab::{self,MeasuredComparison}};

const STUDIO_VERSION:&str=include_str!("../../STUDIO_VERSION");
fn intent_code(i:lcms2::Intent)->u8{match i{lcms2::Intent::Perceptual=>0,lcms2::Intent::RelativeColorimetric=>1,lcms2::Intent::Saturation=>2,lcms2::Intent::AbsoluteColorimetric=>3,_=>1}}
fn intent_from_code(i:u8)->lcms2::Intent{match i{0=>lcms2::Intent::Perceptual,2=>lcms2::Intent::Saturation,3=>lcms2::Intent::AbsoluteColorimetric,_=>lcms2::Intent::RelativeColorimetric}}
fn intent_label(i:lcms2::Intent)->&'static str{match i{lcms2::Intent::Perceptual=>"Perceptual",lcms2::Intent::Saturation=>"Saturation",lcms2::Intent::AbsoluteColorimetric=>"Absolute",_=>"Relative"}}

#[derive(Clone,Copy,PartialEq,Eq)]
enum Page { Inspector, Validator, Rebuilder, Comparison, Curves, Reorder, Version, Gamut, Gradient, Correction, Smoothing, DeviceLink, Recipes, Black, Lab, Settings, About }
#[derive(Clone,Copy,PartialEq,Eq)]
enum Picker{Reference,Comparison,Gamut}
enum Event { Progress(String), Done(Result<String,String>) }
struct Studio {
 link_grid:usize,link_tolerance:f64,link_intent:lcms2::Intent,link_report:Option<(PathBuf,PathBuf,PathBuf,device_link::MultiLinkReport)>,
 slider_drag:Option<(&'static str,usize)>,numeric_edit:Option<((&'static str,usize),String,FocusHandle,bool)>,embedded_names:bool,display_profile:Option<Profile>,language:Language,nav_collapsed:[bool;5],picker:Option<Picker>,system_catalog:Option<system_profiles::Catalog>,system_error:Option<String>,system_loading:bool,
 version_allow_omit:bool,version_relative_only:bool,version_de:f64,version_ink:f64,version_report:Option<versioning::Report>,page:Page, source:Option<PathBuf>, capability:Option<Capabilities>, predicted_inks:Vec<u32>, report:Option<Inspection>, error:Option<String>,
 caps:Vec<f64>,tac:f64,grid:usize,mode:Mode,status:String,busy:bool,analysis_job:Option<color_convert_rust::analysis_job::Job>,rebuild_control:Option<color_convert_rust::multichannel::RebuildControl>, candidate:Option<PathBuf>,comparison:Option<Comparison>,compare_error:Option<String>,compare_grid:usize,compare_intent:lcms2::Intent,gamma:Vec<f64>,order:Vec<usize>, preview_open:bool, preview:Option<Overlay>, preview_revision:u64, preview_width:f32,preview_fullscreen:bool,resize_last:Option<Pixels>,gamut_advanced:bool,curve_drag:Option<(usize,usize,Pixels)>,curve_mode:usize,curve_channel:usize, preview_cache:std::cell::RefCell<gamut_view::PreviewCache>, preview_options:ViewOptions, gamut_grid:usize,gamut_reference_intent:lcms2::Intent,gamut_candidate_intent:lcms2::Intent,gamut_separate_intents:bool,gradient:Option<GradientReport>,gradient_detail:Option<usize>,gradient_selected:usize,gradient_shared_scale:bool,gradient_error:Option<String>,coverage_threshold:f64, regions:[Selective;MAX_REGIONS],region_count:usize,region_selected:usize, correction_grid:usize, correction_tac:f64, smoothing:f64, smoothing_passes:usize,orbit_last:Option<Point<Pixels>>,chart_dpi:u32,recipe_target:[f64;3],recipe_settings:RecipeSettings,recipe_result:Option<RecipeReport>,lab_levels:usize,lab_tab:usize,lab_advanced:bool,lab_loading:bool,lab_attempted:bool,lab_epoch:u64,lab_error:Option<String>,compare_overlay:bool,compare_details:bool,recipe_tab:usize,recipe_weights_open:bool,characterization:Option<color_convert_rust::characterization::Chart>,characterization_page:usize,characterization_patch:Option<usize>,characterization_fit:Option<color_convert_rust::characterization::Fit>,ncolor_report:Option<color_convert_rust::ncolor_model::CrossValidation>,ncolor_inverse:Option<color_convert_rust::ncolor_model::InverseReport>,ncolor_audit:Option<color_convert_rust::ncolor_model::ContinuityAudit>,ncolor_ramp:Option<color_convert_rust::ncolor_model::InverseRampAudit>,ncolor_regularized:Option<color_convert_rust::ncolor_model::RegularizedRamp>,ncolor_lattice:Option<color_convert_rust::ncolor_model::LatticeAudit>,ncolor_forward:Option<color_convert_rust::ncolor_model::ForwardGridAudit>,ncolor_forward_grid:Vec<usize>,ncolor_span:[f64;3],ncolor_slice:usize,ncolor_lambda:f64,ncolor_target:[f64;3],ncolor_tolerance:f64,ncolor_distance:f64,characterization_measured:bool,characterization_heat:bool,spectral_illuminant:Illuminant,spectral_observer:Observer,characterization_filter:target_tools::Filter,characterization_cloud:bool,characterization_view:ViewOptions,characterization_cache:std::cell::RefCell<gamut_view::PreviewCache>,characterization_revision:u64,measured:[Option<PathBuf>;2],measurement_result:Option<MeasuredComparison>,black:BlackGeneration,black_channel:Option<usize>,curve_points:Vec<[f64;5]>,logo:std::sync::Arc<gpui::Image>,#[cfg(windows)] updater:StudioUpdater,#[cfg(windows)] update_started:bool,#[cfg(windows)] update_notice:Option<String>,#[cfg(windows)] notified_version:Option<String>,
}
impl Default for Studio {
 fn default()->Self{Self{link_grid:0,link_tolerance:2.,link_intent:lcms2::Intent::RelativeColorimetric,link_report:None,slider_drag:None,numeric_edit:None,embedded_names:true,display_profile:None,language:Language::En,nav_collapsed:[false;5],picker:None,system_catalog:None,system_error:None,system_loading:false,version_allow_omit:false,version_relative_only:false,version_de:0.5,version_ink:0.5,version_report:None,page:Page::Inspector,source:None,capability:None,predicted_inks:vec![],report:None,error:None,caps:vec![],tac:240.,grid:33,mode:Mode::Custom,status:"Open an ICC or ICM profile to start.".into(),busy:false,analysis_job:None,rebuild_control:None,candidate:None,comparison:None,compare_error:None,compare_grid:17,compare_intent:lcms2::Intent::RelativeColorimetric,gamma:vec![],order:vec![],preview_open:true,preview:None,preview_revision:0,preview_width:360.,preview_fullscreen:false,resize_last:None,gamut_advanced:false,curve_drag:None,curve_mode:0,curve_channel:0,preview_cache:std::cell::RefCell::new(gamut_view::PreviewCache::default()),preview_options:ViewOptions::default(),gamut_grid:33,gamut_reference_intent:lcms2::Intent::RelativeColorimetric,gamut_candidate_intent:lcms2::Intent::RelativeColorimetric,gamut_separate_intents:false,gradient:None,gradient_detail:None,gradient_selected:0,gradient_shared_scale:true,gradient_error:None,coverage_threshold:2.,regions:[Selective::default();MAX_REGIONS],region_count:1,region_selected:0,correction_grid:33,correction_tac:400.,smoothing:0.20,smoothing_passes:1,orbit_last:None,chart_dpi:300,recipe_target:[50.,0.,0.],recipe_settings:RecipeSettings{caps:vec![100.;4],tac:240.,weights:vec![1.;4],tolerance:2.},recipe_result:None,lab_levels:5,lab_tab:0,lab_advanced:false,lab_loading:false,lab_attempted:false,lab_epoch:0,lab_error:None,compare_overlay:true,compare_details:false,recipe_tab:0,recipe_weights_open:false,characterization:None,characterization_page:1,characterization_patch:None,characterization_fit:None,ncolor_report:None,ncolor_inverse:None,ncolor_audit:None,ncolor_ramp:None,ncolor_regularized:None,ncolor_lattice:None,ncolor_forward:None,ncolor_forward_grid:vec![],ncolor_span:[1.;3],ncolor_slice:1,ncolor_lambda:0.02,ncolor_target:[50.,0.,0.],ncolor_tolerance:2.,ncolor_distance:0.4,characterization_measured:false,characterization_heat:false,spectral_illuminant:Illuminant::D50,spectral_observer:Observer::Two,characterization_filter:target_tools::Filter::default(),characterization_cloud:false,characterization_view:ViewOptions{profile_colors:false,layout:CompareLayout::Shared,solid:false,wireframe:false,floor_contour:false,..ViewOptions::default()},characterization_cache:std::cell::RefCell::new(gamut_view::PreviewCache::default()),characterization_revision:0,measured:[None,None],measurement_result:None,black:BlackGeneration::default(),black_channel:None,curve_points:IDENTITY_POINTS.to_vec(),logo:std::sync::Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png,include_bytes!("../../assets/icon.png").to_vec())),#[cfg(windows)] update_started:false,#[cfg(windows)] update_notice:None,#[cfg(windows)] notified_version:None,#[cfg(windows)] updater:StudioUpdater::new(UpdateConfig::new("emadgh/color-convert-rust","ICC-Profile-Studio.exe",STUDIO_VERSION.trim()).with_app_name("ICC Profile Studio").with_checksum_asset("ICC-Profile-Studio.exe.sha256").with_required_checksum(true))}}
}
fn button(id: impl Into<ElementId>,label:impl Into<SharedString>)->Stateful<Div>{
 let label:SharedString=label.into();
 let icon=match label.as_ref(){"Open ICC"|"Select candidate ICC"=>"▱","Website"=>"↗","Source code"=>"⌘","Releases"|"Windows builds"=>"⇩","Check for updates"=>"⟳","Download verified update"=>"⇩","Restart and install"=>"↻","Compare profiles"=>"≍",_=>""};
 let title=if icon.is_empty(){tr(label.as_ref())}else{format!("{icon}  {}",tr(label.as_ref()))};
 div().id(id).px_3().py_2().rounded_md().bg(rgb(0x333333)).text_color(rgb(0xf1f1f1)).cursor_pointer().hover(|s|s.bg(rgb(0x414141))).when(i18n::is_rtl(),|p|p.text_right()).child(title)
}
fn seg(id:impl Into<ElementId>,label:impl Into<SharedString>,active:bool)->Stateful<Div>{
 let label:SharedString=label.into();
 div().id(id).px_2().py_1().text_xs().rounded_md().bg(rgb(if active{0xb95b26}else{0x333333})).text_color(rgb(0xf1f1f1)).cursor_pointer().hover(|s|s.bg(rgb(0x4a4a4a))).child(tr(label.as_ref()))
}
fn heading(title:&str,sub:&str)->Div{div().flex().flex_col().gap_1().when(i18n::is_rtl(),|p|p.text_right()).child(div().text_2xl().child(tr(title))).child(div().text_sm().text_color(rgb(0xa1a1aa)).child(tr(sub)))}
fn visible_ink(color:u32)->u32{if color==0||((color>>16)&255)<32&&((color>>8)&255)<32&&(color&255)<32{0x262626}else{color}}
fn swatch(color:u32,height:f32)->Div{div().h(px(height)).min_w(px(height)).rounded_md().border_1().border_color(rgb(0x777777)).bg(rgb(color))}
fn bar(value:f64,color:u32,height:f32)->Div{
 let value=if value.is_finite(){value.clamp(0.,1.)}else{0.};
 div().w_full().h(px(height)).rounded_md().bg(rgb(0x171717)).overflow_hidden()
  .child(div().w(relative(value.clamp(0.,1.) as f32)).h_full().bg(rgb(color)).when(value>0.&&color==0x262626,|p|p.border_1().border_color(rgb(0x696969))))
}
fn ink_budget_dynamic(caps:&[f64],tac:f64)->Div{
 let max=(caps.len() as f64*100.).max(1.);let sum=caps.iter().sum::<f64>();let effective=sum.min(tac);
 div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828))
  .child(div().text_lg().child(tr("Total ink coverage (TAC)")))
  .child(format!("{} {sum:.0}% · TAC {tac:.0}%",tr("Sum of limits")))
  .child(div().relative().w_full().h(px(22.)).bg(rgb(0x171717)).rounded_md().overflow_hidden()
   .child(div().absolute().left(px(0.)).top(px(0.)).h_full().w(relative((sum/max).clamp(0.,1.) as f32)).bg(rgb(0x484848)))
   .child(div().absolute().left(px(0.)).top(px(0.)).h_full().w(relative((effective/max).clamp(0.,1.) as f32)).bg(rgb(0xe87935)))
   .child(div().absolute().left(relative((tac/max).clamp(0.,1.) as f32)).top(px(0.)).w(px(2.)).h_full().bg(rgb(0xf1f1f1))))
  .child(div().flex().justify_between().text_xs().children((0..=4).map(|i|format!("{:.0}%",max*i as f64/4.))))
  .child(format!("{} ≤ {effective:.0}%",tr("Requested combined ceiling")))
  .child(div().text_xs().child(tr("Live requested limits, not simulated separation.")))
}

/// Shared line chart, draws all samples using the same scale for both profiles.
fn analysis_plot(before:&[f64],after:Option<&[f64]>,scale:f64)->impl IntoElement{
 let series=vec![(ui_geometry::segments(before,scale),0x60a5fa),(ui_geometry::segments(after.unwrap_or(&[]),scale),0xfb923c)];
 canvas(move|_,_,_|{},move|bounds,_,window,_|{
  window.paint_quad(fill(bounds,rgb(0x101010)));
  for i in 0..=4{
   let t=i as f32/4.;let x=bounds.origin.x+(bounds.size.width-px(1.))*t;let y=bounds.origin.y+(bounds.size.height-px(1.))*t;
   window.paint_quad(fill(Bounds{origin:point(x,bounds.origin.y),size:size(px(1.),bounds.size.height)},rgb(0x393939)));
   window.paint_quad(fill(Bounds{origin:point(bounds.origin.x,y),size:size(bounds.size.width,px(1.))},rgb(0x393939)));
  }
  for (segments,color) in &series{
   for &[a,b] in segments{
    let n=ui_geometry::line_steps(a,b,f32::from(bounds.size.width),f32::from(bounds.size.height));
    for j in 0..=n{
     let t=j as f32/n as f32;let x=a[0]+(b[0]-a[0])*t;let y=a[1]+(b[1]-a[1])*t;
     let stroke=if *color==0x60a5fa{2.8}else{1.4};
     let p=point(bounds.origin.x+px(2.-stroke/2.)+(bounds.size.width-px(4.))*x,bounds.origin.y+px(2.-stroke/2.)+(bounds.size.height-px(4.))*(1.-y));
     window.paint_quad(fill(Bounds{origin:p,size:size(px(stroke),px(stroke))},rgb(*color)));
    }
   }
  }
 }).w_full().h(px(110.))
}
fn chart_labels(max:f64)->Div{div().flex().justify_between().text_xs().text_color(rgb(0xa1a1aa)).child("0").child(format!("Y: 0–{max:.2}")).child("255")}
fn maximum_for_chart(c:&color_convert_rust::characterization::Chart)->f64{c.patches.first().map(|p|p.inks.len() as f64*100.).unwrap_or(400.)}
fn comparison_pair_plot(label:&str,a:&[f64],b:&[f64],overlay:bool)->Div{
 let mut card=div().flex().flex_col().gap_1().p_2().rounded_md().bg(rgb(0x282828)).child(label.to_owned());
 if overlay{
  card=card.child(analysis_plot(a,Some(b),100.)).child(chart_labels(100.));
 }else{
  let mut pair=div().flex().gap_2();
  for (side,values,color) in [(0,a,0x60a5fa),(1,b,0xfb923c)]{
   let mut lane=div().flex_1().min_w(px(0.)).flex().flex_col().gap_1().child(div().text_xs().text_color(rgb(color)).child(tr(if side==0{"Reference"}else{"Candidate"})));
   if values.is_empty(){lane=lane.child(div().h(px(110.)).bg(rgb(0x191919)).child(tr("No channel at this index")));}else{lane=lane.child(if side==0{analysis_plot(values,None,100.)}else{analysis_plot(&[],Some(values),100.)});}
   pair=pair.child(lane.child(chart_labels(100.)));
  }
  card=card.child(pair);
 }
 card
}
fn lab_swatch(lab:[f64;3])->u32{let [r,g,b]=color_convert_rust::gamut_raster::lab_rgb(lab.map(|x|x as f32));(u32::from(r)<<16)|(u32::from(g)<<8)|u32::from(b)}

impl Studio {
 fn persist_name_mode(&mut self){if let Err(e)=preferences::location().and_then(|p|{let mut current=preferences::load(&p)?;current.embedded_names=self.embedded_names;preferences::save(&p,&current)}){self.status=format!("Name preference not saved: {e}");}}
 fn ink_names(&self)->Vec<String>{self.display_profile.as_ref().map(|p|if self.embedded_names{p.channels()}else{p.standard_channels()}).unwrap_or_default()}
 fn ink_name(&self,i:usize)->String{self.ink_names().get(i).cloned().map(|n|if self.embedded_names{color_convert_rust::rtl::visual(&n)}else{tr(&n)}).unwrap_or_else(||format!("CH {}",i+1))}
 fn ink_color(&self,i:usize)->u32{visible_ink(self.display_profile.as_ref().map(|p|capabilities::channel_display(p,i,&self.predicted_inks,self.embedded_names).0).unwrap_or(0x858585))}
 fn ink_provenance(&self,i:usize)->String{self.display_profile.as_ref().map(|p|tr(capabilities::channel_display(p,i,&self.predicted_inks,self.embedded_names).1.label())).unwrap_or_default()}
 fn page_reason(&self,page:Page)->Option<&str>{let c=self.capability.as_ref()?;let t=match page{Page::Rebuilder=>Tool::Rebuild,Page::Reorder=>Tool::Reorder,Page::Correction|Page::Smoothing=>Tool::Correction,Page::Black=>Tool::Optimization,Page::Comparison|Page::Gamut|Page::Gradient=>Tool::Analysis,Page::Version=>Tool::Version,Page::DeviceLink=>Tool::DeviceLink,_=>return None};c.reason(t)}

 fn numeric_slider(&self,id:(&'static str,usize),value:f64,lo:f64,hi:f64,step:f64,set:impl Fn(&mut Self,f64)+Copy+'static,cx:&mut Context<Self>)->impl IntoElement{
  let bounds=std::rc::Rc::new(std::cell::Cell::new(None::<Bounds<Pixels>>));
  let measured=bounds.clone();let down=bounds.clone();let motion=bounds.clone();
  let scale=Scale{low:lo,high:hi,step};let ratio=scale.fraction(value) as f32;
  let get=move|b:Bounds<Pixels>,x:Pixels|{
   let t=f32::from(x-b.origin.x-px(7.))/f32::from(b.size.width-px(14.)).max(1.);
   scale.at(t.clamp(0.,1.) as f64)
  };
  let busy=self.busy;
  let track=div().id("numeric-track").relative().w(px(116.)).h(px(26.)).flex_shrink_0().cursor_pointer()
   .child(canvas(move|b,_,_|{measured.set(Some(b));},move|b,_,window,_|{
    let w=(b.size.width-px(14.)).max(px(1.));let x=b.origin.x+px(7.);let y=b.origin.y+px(11.);
    window.paint_quad(fill(Bounds{origin:point(x,y),size:size(w,px(4.))},rgb(0x565656)));
    window.paint_quad(fill(Bounds{origin:point(x,y),size:size(w*ratio,px(4.))},rgb(if busy{0x777777}else{0xe87935})));
    window.paint_quad(fill(Bounds{origin:point(x+w*ratio-px(5.),b.origin.y+px(7.)),size:size(px(10.),px(12.))},rgb(if busy{0x858585}else{0xf1f1f1})));
   }).size_full())
   .on_mouse_down(MouseButton::Left,cx.listener(move|v,e:&MouseDownEvent,_,cx|{if !v.busy{if let Some(b)=down.get(){v.numeric_edit=None;v.slider_drag=Some(id);set(v,get(b,e.position.x));cx.notify();cx.stop_propagation();}}}))
   .on_mouse_move(cx.listener(move|v,e:&MouseMoveEvent,_,cx|{if !v.busy&&e.dragging()&&v.slider_drag==Some(id){if let Some(b)=motion.get(){set(v,get(b,e.position.x));cx.notify();}}}))
   .on_mouse_up(MouseButton::Left,cx.listener(|v,_,_,_|{v.slider_drag=None;})).on_mouse_up_out(MouseButton::Left,cx.listener(|v,_,_,_|{v.slider_drag=None;}));
  // Unique parent identity prevents repeated +/- controls from sharing state.
  let mut control=div().id(id).flex().flex_wrap().items_center().gap_1().flex_shrink_0();
  let bare=matches!(id.0,"target-min"|"target-max"|"target-tac"|"target-yaw"|"target-pitch"|"link-tolerance"|"recipe-tolerance-slider"|"ncolor-target"|"ncolor-inverse-limit"|"ncolor-continuity-weight"|"ncolor-span"|"ncolor-forward-grid"|"rebuilder-tac"|"rebuild-cap");
  if bare{control=control.child(seg("precise-minus","−",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.numeric_edit=None;set(v,scale.adjust(value,-1));cx.notify();}})));}
  control=control.child(track);
  if bare{control=control.child(seg("precise-plus","+",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.numeric_edit=None;set(v,scale.adjust(value,1));cx.notify();}})));}
  if let Some((_,buffer,focus,invalid))=self.numeric_edit.as_ref().filter(|(key,_,_,_)|*key==id){
   let typed=buffer.clone();let focus=focus.clone();
   control=control.child(div().id("number-entry").track_focus(&focus).px_2().py_1().min_w(px(56.)).border_1().border_color(rgb(if *invalid{0xf87171}else{0xe87935})).rounded_md().child(if typed.is_empty(){"…".into()}else{typed})
    .on_key_down(cx.listener(move|v,e:&KeyDownEvent,window,cx|{
     if v.busy{v.numeric_edit=None;cx.notify();return}
     let Some((key,buf,_,bad))=v.numeric_edit.as_mut()else{return};if *key!=id{return}
     if e.keystroke.modifiers.control&&!e.keystroke.modifiers.alt{
      match e.keystroke.key.as_str(){
       "v"=>{if let Some(text)=cx.read_from_clipboard().and_then(|item|item.text()).and_then(|t|ui_geometry::normalize_numeric(&t)){*buf=text;*bad=scale.parse(buf).is_none();}else{*bad=true;}},
       "c"=>{if !buf.is_empty(){cx.write_to_clipboard(ClipboardItem::new_string(buf.clone()));}},
       "a"=>{buf.clear();*bad=false;},
       _=>{}
      }
      window.prevent_default();cx.stop_propagation();cx.notify();return
     }
     match e.keystroke.key.as_str(){
      "enter"=>{if let Some(x)=scale.parse(buf){v.numeric_edit=None;set(v,x);}else{*bad=true;}},
      "escape"=>{v.numeric_edit=None;},
      "backspace"=>{buf.pop();*bad=false;},
      "delete"=>{buf.clear();*bad=false;},
      _=>{if let Some(c)=e.keystroke.key_char.as_deref(){if buf.chars().count()+c.chars().count()<=24&&c.chars().all(|x|matches!(x,'0'..='9'|'۰'..='۹'|'٠'..='٩'|'.'|','|'٫'|'-'|'−'|'+')){buf.push_str(c);*bad=false;}}}
     }
     window.prevent_default();cx.stop_propagation();cx.notify();
    })));
   control=control.child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Numeric input: Ctrl+V pastes; Ctrl+A clears; Enter applies; Escape cancels.")));
   if *invalid{control=control.child(div().text_xs().text_color(rgb(0xf87171)).child(tr("Invalid numeric value or outside allowed range.")));}
  }else{
   control=control.child(seg("numeric-edit",scale.label(value),false).on_click(cx.listener(move|v,_,window,cx|{
    if !v.busy{let focus=cx.focus_handle();window.focus(&focus);v.numeric_edit=Some((id,String::new(),focus,false));cx.notify();}
   })));
  }
  control
 }
 fn toggle_system_picker(&mut self,target:Picker,cx:&mut Context<Self>){
  if self.busy{return}
  if self.picker==Some(target){self.picker=None;cx.notify();return}
  self.picker=Some(target);
  if !self.system_loading&&self.system_catalog.is_none(){self.scan_system(cx);}cx.notify();
 }
 fn scan_system(&mut self,cx:&mut Context<Self>){
  if self.system_loading{return}
  self.system_loading=true;self.system_error=None;self.system_catalog=None;cx.notify();
  cx.spawn(async move|this,cx|{
   let result=cx.background_spawn(async move{system_profiles::directory().and_then(|p|system_profiles::scan(&p))}).await;
   let _=this.update(cx,|v,cx|{v.system_loading=false;match result{Ok(c)=>v.system_catalog=Some(c),Err(e)=>v.system_error=Some(e)}cx.notify();});
  }).detach();
 }
 fn system_dropdown(&self,target:Picker,cx:&mut Context<Self>)->Div{
  let key=match target{Picker::Reference=>"sys-ref-button",Picker::Comparison=>"sys-compare-button",Picker::Gamut=>"sys-gamut-button"};
  let selected=if target==Picker::Reference{self.source.as_ref()}else{self.candidate.as_ref()};
  let rtl=self.language.rtl();
  let label=selected.and_then(|p|p.file_name()).map(|p|{let name=p.to_string_lossy();let mut label=name.chars().take(20).collect::<String>();if name.chars().count()>20{label.push('…');}label}).unwrap_or_else(||tr("System profiles"));
  div().relative().flex_shrink_0().when(target==Picker::Reference,|p|p.w_full()).child(div().id(key).flex().items_center().justify_between().gap_2().px_2().py_2().rounded_md().border_1().border_color(rgb(if self.picker==Some(target){0xe87935}else{0x626262})).bg(rgb(0x262626)).hover(|p|p.bg(rgb(0x333333))).cursor_pointer().when(rtl,|p|p.flex_row_reverse()).child(div().flex_1().min_w(px(0.)).when(rtl,|p|p.text_right()).child(label)).child(div().text_color(rgb(0xa1a1aa)).child(if self.picker==Some(target){"▴"}else{"▾"})).on_click(cx.listener(move|v,_,_,cx|v.toggle_system_picker(target,cx))))
   .when(self.picker==Some(target),|p|p.child(div().absolute().left(px(0.)).top(px(35.)).child(deferred(anchored().snap_to_window().child(self.system_picker(target,cx))).with_priority(10))))
 }
 fn system_picker(&self,target:Picker,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_1().p_2().rounded_md().bg(rgb(0x262626)).w(px(360.)).border_1().border_color(rgb(0x666666)).flex_shrink_0().occlude().when(self.language.rtl(),|p|p.text_right())
   .child(div().flex().items_center().justify_between().when(self.language.rtl(),|p|p.flex_row_reverse()).child(div().text_xs().child(tr("System profiles")))
    .child(div().flex().gap_1().child(seg("sys-reload","↻",false).on_click(cx.listener(|v,_,_,cx|v.scan_system(cx))))
     .child(seg("sys-close","×",false).on_click(cx.listener(|v,_,_,cx|{v.picker=None;cx.notify();})))));
  if self.system_loading{return p.child(div().text_xs().child(tr("Reading Windows color directory…")))}
  if let Some(e)=&self.system_error{return p.child(div().text_xs().child(i18n::diagnostic(e)))}
  if let Some(c)=&self.system_catalog{
   let count=c.entries.iter().filter(|e|target==Picker::Reference||!e.device_link).count();
   p=p.child(div().text_xs().text_color(rgb(0xa1a1aa)).child(format!("{count} profiles · {} excluded · {} invalid",c.excluded,c.invalid)));
   if count==0{return p.child(div().text_xs().child(tr("No matching installed profiles.")))}
   p=p.child(div().id("sys-profile-list").max_h(px(250.)).overflow_y_scroll().flex().flex_col().gap_1()
    .children(c.entries.iter().enumerate().filter(|(_,e)|target==Picker::Reference||!e.device_link).map(|(i,e)|{
     let path=e.path.clone();let chosen=if target==Picker::Reference{self.source.as_ref()==Some(&path)}else{self.candidate.as_ref()==Some(&path)};
     div().id(("system-profile",i)).p_2().rounded_md().bg(rgb(if chosen{0x684024}else{0x333333})).hover(|s|s.bg(rgb(0x414141))).cursor_pointer()
      .child(div().flex().justify_between().text_xs().child(e.name.clone()).when(chosen,|p|p.child("✓"))).child(div().text_xs().text_color(rgb(0xa1a1aa)).child(e.format.clone()))
      .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(e.description.clone()))
      .on_click(cx.listener(move|v,_,_,cx|{
       if v.busy{return}v.picker=None;
       if target==Picker::Reference{v.load_reference(path.clone(),cx);}
       else{v.candidate=Some(path.clone());v.preview_options.candidate=true;v.preview=None;v.comparison=None;v.compare_error=None;v.gradient=None;v.gradient_detail=None;v.gradient_error=None;if v.preview_open||target==Picker::Gamut{v.refresh_gamut(cx);}}
       cx.notify();
      }))
    })));
  }
  p
 }
 fn open(&mut self,cx:&mut Context<Self>) {
  if self.busy{return}
  let Some(path)=rfd::FileDialog::new().add_filter("ICC profile",&["icc","icm"]).pick_file() else{return};
  self.load_reference(path,cx);
 }
 fn load_reference(&mut self,path:PathBuf,cx:&mut Context<Self>){
  if self.busy{return}
  self.numeric_edit=None;self.picker=None;
  let retained_candidate=if matches!(self.page,Page::Comparison|Page::DeviceLink|Page::Gradient){self.candidate.clone()}else{None};
  self.lab_epoch=self.lab_epoch.wrapping_add(1);self.lab_loading=false;self.lab_attempted=false;self.lab_error=None;self.measured=[None,None];self.measurement_result=None;self.link_report=None;
  self.source=Some(path.clone());self.characterization=None;self.ncolor_report=None;self.ncolor_inverse=None;self.ncolor_audit=None;self.ncolor_ramp=None;self.ncolor_regularized=None;self.ncolor_lattice=None;self.ncolor_forward=None;self.ncolor_forward_grid.clear();self.characterization_patch=None;self.characterization_fit=None;self.characterization_measured=false;self.characterization_heat=false;self.spectral_illuminant=Illuminant::D50;self.spectral_observer=Observer::Two;self.characterization_filter=target_tools::Filter::default();self.characterization_cloud=false;self.characterization_revision=self.characterization_revision.wrapping_add(1);self.characterization_page=1;self.version_report=None;self.version_allow_omit=false;self.version_relative_only=false;self.black=BlackGeneration::default();self.black_channel=None;self.curve_points=IDENTITY_POINTS.to_vec();self.recipe_result=None;self.recipe_settings.weights.clear();self.regions=[Selective::default();MAX_REGIONS];self.region_count=1;self.region_selected=0;self.correction_grid=33;self.correction_tac=400.;self.smoothing=0.20;self.smoothing_passes=1;self.preview=None;self.candidate=retained_candidate;self.gradient=None;self.gradient_detail=None;self.gradient_error=None;self.gamma.clear();self.order.clear();self.curve_drag=None;self.curve_channel=0;self.comparison=None;self.compare_error=None;self.report=None;self.capability=None;self.predicted_inks.clear();self.display_profile=None;self.caps.clear();self.tac=0.;self.mode=Mode::Custom;self.error=None;
  match inspection::inspect(&path){Ok(r)=>self.report=Some(r),Err(e)=>self.error=Some(e)}
  if let Ok(p)=Profile::read(&path){
   self.display_profile=Some(p.clone());
   self.capability=Some(capabilities::inspect(&p));self.predicted_inks=capabilities::predicted_swatches(&p).unwrap_or_default();
   if let Ok(n)=editing::editable_channel_count(&p){self.gamma=vec![1.;n];self.recipe_settings.weights=vec![1.;n];self.order=(0..n).collect();self.curve_points=vec![[0.,0.25,0.5,0.75,1.];n];if !p.cmyk(){self.correction_tac=n as f64*100.;}}
   let n=self.capability.as_ref().map(|c|c.channels).unwrap_or(0);
   if n>0{
    self.caps=vec![100.;n];
    if p.cmyk(){self.caps[3]=0.;}
    self.tac=self.report.as_ref().and_then(|r|r.tac).unwrap_or((n as f64*100.).min(240.)).clamp(0.,n as f64*100.);
    self.mode=if p.version_major()==2&&p.cmyk(){Mode::Argyll}else{Mode::Custom};
    self.grid=if p.version_major()==4&&p.cmyk(){33}else{17};
   }
  }
  self.status=if self.report.is_some(){"Profile loaded with LittleCMS.".into()}else{"Profile could not be opened.".into()};
  if self.page==Page::Lab{self.ensure_lab_target(cx);}
  if self.preview_open && self.source.is_some(){self.refresh_gamut(cx);}
 }
 fn generate(&mut self,cx:&mut Context<Self>){
  if self.busy||self.caps.is_empty(){return}
  if let Some(reason)=self.page_reason(Page::Rebuilder){self.status=reason.to_owned();cx.notify();return}
  let Some(source)=self.source.clone()else{return};
  let name=format!("{}_limited.icc",source.file_stem().unwrap_or_default().to_string_lossy());
  let Some(mut destination)=rfd::FileDialog::new().add_filter("ICC profile",&["icc","icm"]).set_file_name(name).save_file()else{return};
  if destination.extension().is_none(){destination.set_extension("icc");}
  let path=destination.clone();let caps=self.caps.clone();let tac=self.tac;let grid=self.grid;
  let argyll=self.mode==Mode::Argyll&&caps.len()==4&&caps[..3].iter().all(|&v|v==100.);
  let control=color_convert_rust::multichannel::RebuildControl::default();self.rebuild_control=if argyll{None}else{Some(control.clone())};
  let (tx,rx)=mpsc::channel();self.busy=true;self.status="Starting multichannel rebuild…".into();cx.notify();
  std::thread::spawn(move||{
   let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{let progress=|s|{let _=tx.send(Event::Progress(s));};if argyll{let fixed:[f64;4]=caps.clone().try_into().map_err(|_|"Argyll requires four channels".to_owned())?;backend::run(Job{source,destination,mode:Mode::Argyll,caps:fixed,tac,estimate:false,grid},progress)}else{backend::run_multichannel_controlled(&source,&destination,&caps,tac,grid,&control,progress)}})).unwrap_or_else(|_|Err("Multichannel rebuild panicked".into()));
   let _=tx.send(Event::Done(result));
  });
  cx.spawn(async move|this,cx|{
   loop{
    match rx.try_recv(){
     Ok(Event::Progress(s))=>{if this.update(cx,|v,cx|{if !v.rebuild_control.as_ref().is_some_and(|c|c.cancelled()){v.status=s;cx.notify();}}).is_err(){break}}
     Ok(Event::Done(result))=>{let _=this.update(cx,|v,cx|{v.busy=false;v.rebuild_control=None;match result{Ok(s)=>{v.status=s;v.candidate=Some(path.clone());v.preview=None;v.comparison=None;v.gradient=None;v.gradient_detail=None;v.gradient_error=None;if v.preview_open{v.refresh_gamut(cx);}},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});break}
     Err(mpsc::TryRecvError::Disconnected)=>{let _=this.update(cx,|v,cx|{v.busy=false;v.rebuild_control=None;v.status="Worker disconnected".into();cx.notify();});break}
     Err(mpsc::TryRecvError::Empty)=>{}
    }
    cx.background_executor().timer(Duration::from_millis(70)).await;
   }
  }).detach();
 }

 fn start_analysis_job(&mut self,cx:&mut Context<Self>)->color_convert_rust::analysis_job::Job{
  self.numeric_edit=None;let job=color_convert_rust::analysis_job::Job::default();self.analysis_job=Some(job.clone());let poll=job.clone();
  cx.spawn(async move|this,cx|{loop{
   cx.background_executor().timer(Duration::from_millis(120)).await;
   if !matches!(this.update(cx,|v,cx|{let active=v.analysis_job.as_ref().is_some_and(|j|j.same(&poll));if active{cx.notify();}active}),Ok(true)){break}
  }}).detach();
  job
 }
 fn finish_analysis_job(&mut self,job:&color_convert_rust::analysis_job::Job,cx:&mut Context<Self>)->bool{
  if !self.analysis_job.as_ref().is_some_and(|j|j.same(job)){return false}
  self.analysis_job=None;self.busy=false;
  if job.cancelled(){self.status="Analysis cancelled".into();cx.notify();return false}true
 }
 fn analysis_progress(&self,scope:usize,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_1();
  if let Some(j)=&self.analysis_job{
   let x=j.progress();
   p=p.child(div().flex().flex_wrap().items_center().gap_2()
    .child(div().flex_1().text_xs().child(if x.phase.is_empty(){tr("Working…")}else{tr(x.phase)}))
    .child(button(("cancel-analysis",scope),if x.cancelled{tr("Cancelling…")}else{tr("Cancel analysis")}).on_click(cx.listener(|v,_,_,cx|{if let Some(j)=&v.analysis_job{j.cancel();cx.notify();}}))));
   if x.total>0{p=p.child(bar(x.completed as f64/x.total as f64,0xe87935,6.)).child(format!("{} / {} · {:.0}%",x.completed,x.total,100.*x.completed as f64/x.total as f64));}
   else{p=p.child(format!("{}: {}",tr("Completed checkpoints"),x.checkpoints));}
  }p
 }
 fn compare_profiles(&mut self,cx:&mut Context<Self>){
  if self.busy{return}
  let (Some(a),Some(b))=(self.source.clone(),self.candidate.clone())else{return};
  let grid=self.compare_grid;let intent=self.compare_intent;
  self.busy=true;self.comparison=None;self.compare_error=None;self.status="Comparing profile predictions with LittleCMS...".into();cx.notify();
  let job=self.start_analysis_job(cx);cx.spawn(async move |this,cx|{
   let worker=job.clone();let result=cx.background_spawn(async move{worker.run(||comparison::compare(&a,&b,grid,intent))}).await;
   let _=this.update(cx,|view,cx|{if !view.finish_analysis_job(&job,cx){return}
    match result{Ok(r)=>{view.status=format!("Compared {} sRGB samples and 33 neutrals.",r.count);view.comparison=Some(r)},Err(e)=>{view.status="Comparison failed".into();view.compare_error=Some(e)}}
    view.busy=false;cx.notify();
   });
  }).detach();
 }
 fn comparison_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Profile Comparison","Predicted D50 media-relative Lab appearance and CMYK / nCLR separation."));
  let filename=|p:&Option<PathBuf>|p.as_ref().map(|p|p.display().to_string()).unwrap_or_else(||tr("Not selected"));
  p=p.child(div().flex().gap_2()
   .child(div().flex_1().min_w(px(0.)).flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828))
    .child(div().text_color(rgb(0x60a5fa)).child(tr("Reference"))).child(div().text_xs().child(filename(&self.source)))
    .child(button("compare-ref-open","Open ICC").on_click(cx.listener(|v,_,_,cx|v.open(cx)))))
   .child(div().flex_1().min_w(px(0.)).flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828))
    .child(div().text_color(rgb(0xfb923c)).child(tr("Candidate"))).child(div().text_xs().child(filename(&self.candidate)))
    .child(div().flex().flex_wrap().gap_1().child(button("pick-candidate","Select candidate ICC").on_click(cx.listener(|v,_,_,cx|{
     if !v.busy{if let Some(path)=rfd::FileDialog::new().add_filter("ICC profile",&["icc","icm"]).pick_file(){v.candidate=Some(path);v.preview=None;v.comparison=None;v.gradient=None;v.gradient_detail=None;v.gradient_error=None;v.compare_error=None;if v.preview_open{v.refresh_gamut(cx);}cx.notify();}}
    }))).child(self.system_dropdown(Picker::Comparison,cx)))))
   .child(div().flex().items_center().gap_2().child(format!("RGB sample grid: {}³",self.compare_grid)).children([9usize,17,33].into_iter().map(|v|button(("compare-grid",v),v.to_string()).on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.compare_grid=v;this.comparison=None;cx.notify();}})))))
   .child(div().flex().flex_wrap().gap_1().children([("Relative",lcms2::Intent::RelativeColorimetric),("Perceptual",lcms2::Intent::Perceptual),("Saturation",lcms2::Intent::Saturation),("Absolute",lcms2::Intent::AbsoluteColorimetric)].into_iter().enumerate().map(|(i,(n,intent))|button(("compare-intent",i),if self.compare_intent==intent{format!("{n} ✓")}else{n.to_owned()}).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.compare_intent=intent;v.comparison=None;v.compare_error=None;cx.notify();}})))))
   .child(button("compare-run",if self.busy{"Working…"}else{"Compare profiles"}).on_click(cx.listener(|this,_,_,cx|this.compare_profiles(cx))));
  if let Some(e)=&self.compare_error{p=p.child(div().text_color(rgb(0xfca5a5)).child(i18n::diagnostic(e)));}
  if let Some(r)=&self.comparison{
   p=p.child(button("export-comparison","Export analysis CSV").on_click(cx.listener(|v,_,_,cx|{if !v.busy{if let Some(r)=&v.comparison{if let Some(path)=rfd::FileDialog::new().add_filter("CSV",&["csv"]).set_file_name("comparison.csv").save_file(){v.status=reports::comparison(&path,r).map(|_|format!("Saved {}",path.display())).unwrap_or_else(|e|format!("Error: {e}"));cx.notify();}}}})));
   p=p.child(div().flex().gap_2().flex_wrap().children([("Mean ΔE00",r.mean_de),("P95 ΔE00",r.p95_de),("Max ΔE00",r.max_de)].map(|(label,v)|div().flex_1().min_w(px(100.)).p_3().rounded_md().bg(rgb(0x282828)).child(div().text_xs().child(label)).child(div().text_2xl().child(format!("{v:.2}"))))))
    .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(format!("{} samples · worst source RGB {:?}",r.count,r.worst_rgb)))
    .child(div().flex().items_center().gap_2().flex_wrap()
      .child(seg("compare-split","Side by side",!self.compare_overlay).on_click(cx.listener(|v,_,_,cx|{v.compare_overlay=false;cx.notify();})))
      .child(seg("compare-overlay","Overlay",self.compare_overlay).on_click(cx.listener(|v,_,_,cx|{v.compare_overlay=true;cx.notify();})))
      .child(swatch(0x60a5fa,12.)).child(tr("Reference")).child(swatch(0xfb923c,12.)).child(tr("Candidate")))
    .child(div().text_lg().child(tr("Delta E distribution")));
   let mut histogram=div().flex().gap_2().items_end().h(px(125.)).p_2().rounded_md().bg(rgb(0x282828));
   for (i,label) in ["< 1","1–2","2–3","3–5","≥ 5"].into_iter().enumerate(){
    let fraction=r.histogram[i] as f32/r.count.max(1) as f32;
    histogram=histogram.child(div().flex_1().min_w(px(0.)).flex().flex_col().items_center().gap_1().child(div().text_xs().child(format!("{:.1}%",100.*fraction))).child(div().w_full().h(px((78.*fraction).max(1.))).rounded_md().bg(rgb(0x8d8d8d))).child(div().text_xs().child(label)));
   }
   p=p.child(histogram).child(div().text_lg().child(tr("Ink distribution (sampled)")))
    .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Channels are aligned by physical index for viewing only; matching ink identity is not assumed.")));
   let n=r.reference_channels.len().max(r.candidate_channels.len());
   let tac_scale=(n.max(1)*100) as f64;
   let mut total=div().flex().gap_2();
   for (label,stats,color) in [("Reference",&r.reference,0x60a5fa),("Candidate",&r.candidate,0xfb923c)]{
    total=total.child(div().flex_1().min_w(px(0.)).flex().flex_col().gap_1().p_3().rounded_md().bg(rgb(0x282828)).child(div().text_color(rgb(color)).child(tr(label))).child(format!("{} {:.1}%",tr("Mean TAC"),stats.mean_tac)).child(bar(stats.mean_tac/tac_scale,color,9.)).child(format!("{} {:.1}%",tr("Max TAC"),stats.max_tac)).child(bar(stats.max_tac/tac_scale,color,5.)));
   }
   p=p.child(total);
   for j in 0..n{
    let mut row=div().flex().flex_col().gap_1().p_2().rounded_md().bg(rgb(0x282828)).child(div().text_xs().child(format!("{} {}",tr("Channel index"),j+1)));
    let mut lanes=div().flex().gap_2();
    if self.compare_overlay{lanes=lanes.flex_col();}
    for (stats,channels,color) in [(&r.reference,&r.reference_channels,0x60a5fa),(&r.candidate,&r.candidate_channels,0xfb923c)]{
     let mut cell=div().flex_1().min_w(px(0.)).flex().flex_col().gap_1();
     if let Some(ch)=channels.get(j){
      cell=cell.child(div().flex().items_center().gap_1().child(swatch(visible_ink(ch.swatch(self.embedded_names)),12.)).child(div().flex_1().child(color_convert_rust::rtl::visual(ch.label(self.embedded_names)))).child(div().text_xs().child(format!("{:.1} / {:.1}%",stats.mean[j],stats.max[j]))))
       .child(bar(stats.mean[j]/100.,color,10.)).child(bar(stats.max[j]/100.,color,4.));
     }else{cell=cell.child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("No channel at this index")));}
     lanes=lanes.child(cell);
    }
    p=p.child(row.child(lanes));
   }
   p=p.child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Thick bar: mean · thin bar: maximum · shared 0–100% scale.")))
    .child(div().text_lg().child(tr("Neutral axis — all 33 samples")));
   let light_a=r.neutral.iter().map(|v|v.reference_lab[0]).collect::<Vec<_>>();let light_b=r.neutral.iter().map(|v|v.candidate_lab[0]).collect::<Vec<_>>();
   p=p.child(comparison_pair_plot("L*",&light_a,&light_b,self.compare_overlay));
   for j in 0..n{
    let a=r.neutral.iter().filter_map(|v|v.reference_ink.get(j).copied()).collect::<Vec<_>>();
    let b=r.neutral.iter().filter_map(|v|v.candidate_ink.get(j).copied()).collect::<Vec<_>>();
    let label_a=r.reference_channels.get(j).map(|ch|color_convert_rust::rtl::visual(ch.label(self.embedded_names))).unwrap_or_else(||"—".into());
    let label_b=r.candidate_channels.get(j).map(|ch|color_convert_rust::rtl::visual(ch.label(self.embedded_names))).unwrap_or_else(||"—".into());
    p=p.child(comparison_pair_plot(&format!("CH{} · {} / {}",j+1,label_a,label_b),&a,&b,self.compare_overlay));
   }
   p=p.child(seg("compare-details",if self.compare_details{"Hide sample details"}else{"Show sample details"},self.compare_details).on_click(cx.listener(|v,_,_,cx|{v.compare_details=!v.compare_details;cx.notify();})));
   if self.compare_details{for n in &r.neutral{
    p=p.child(div().p_2().rounded_md().bg(rgb(0x282828)).child(format!("Gray {:3} · ΔE00 {:.3}",n.rgb,n.delta_e))
     .child(div().flex().gap_2().child(div().flex_1().min_w(px(0.)).text_xs().child(format!("Lab {:.2?} · ink {:.2?}",n.reference_lab,n.reference_ink))).child(div().flex_1().min_w(px(0.)).text_xs().child(format!("Lab {:.2?} · ink {:.2?}",n.candidate_lab,n.candidate_ink)))));
   }}
   p=p.child(div().text_xs().text_color(rgb(0xa1a1aa)).child(format!("Separation intent: {:?}; predicted appearance evaluated media-relative D50; no BPC. Predicted differences only, not measured print, physical gamut or production ink averages.",r.intent)));
  }
  p
 }
 fn save_edit(&mut self,kind:u8,cx:&mut Context<Self>){
  if self.busy{return}let Some(source)=self.source.clone()else{return};
  let tool=match kind{0=>Some(Tool::Gamma),1=>Some(Tool::Reorder),3|4|7=>Some(Tool::Correction),5|6=>Some(Tool::Optimization),_=>None};if let Some(reason)=tool.and_then(|t|self.capability.as_ref().and_then(|c|c.reason(t))){self.status=reason.to_owned();cx.notify();return}
  if kind==6&&self.black_channel.is_none_or(|k|k>=self.gamma.len()){self.status="Select the physical black ink explicitly.".into();cx.notify();return}
  let suffix=match kind{0=>"curves",1=>"reordered",3=>"selective",4=>"smoothed",5=>"ink_optimized",6=>"black_generation",7=>"point_curves",_=>"renamed"};
  let default_name=format!("{}_{}.icc",source.file_stem().unwrap_or_default().to_string_lossy(),suffix);
  let Some(mut destination)=rfd::FileDialog::new().add_filter("ICC profile",&["icc","icm"]).set_file_name(default_name).save_file()else{return};
  if destination.extension().is_none(){destination.set_extension("icc");}
  let output_path=destination.clone();
  let dynamic=self.display_profile.as_ref().is_some_and(|p|!p.cmyk());
  let edit=match kind{0=>Edit::ToneMulti(self.gamma.clone()),1=>Edit::ReorderMulti(self.order.clone()),3=>Edit::CorrectionMulti(DynamicCorrection::Regions{regions:self.regions,count:self.region_count},self.correction_grid,self.correction_tac),4=>Edit::CorrectionMulti(DynamicCorrection::Smooth{strength:self.smoothing,passes:self.smoothing_passes},self.correction_grid,self.correction_tac),5=>Edit::CorrectionMulti(DynamicCorrection::Optimize{weights:self.recipe_settings.weights.clone(),tolerance:self.recipe_settings.tolerance,caps:self.caps.clone()},self.correction_grid.min(33),self.correction_tac.min(self.tac)),6=>Edit::CorrectionMulti(DynamicCorrection::Black{channel:self.black_channel.unwrap(),settings:self.black,caps:self.caps.clone()},self.correction_grid.min(33),self.correction_tac.min(self.tac)),7=>Edit::CorrectionMulti(DynamicCorrection::Points(self.curve_points.clone()),self.correction_grid,self.correction_tac),_=>Edit::Rename(destination.file_stem().unwrap_or_default().to_string_lossy().into_owned())};
  self.busy=true;self.status="Editing ICC and validating with LittleCMS…".into();cx.notify();
  cx.spawn(async move |this,cx|{
   let result=cx.background_spawn(async move{editing::edit_file(&source,&destination,edit)}).await;
   let _=this.update(cx,|view,cx|{view.busy=false;match result{Ok(s)=>{view.status=s;view.candidate=Some(output_path);view.preview=None;view.comparison=None;view.gradient=None;view.gradient_detail=None;view.gradient_error=None;if view.preview_open{view.refresh_gamut(cx);}},Err(e)=>view.status=format!("Error: {e}")}cx.notify();});
  }).detach();
 }
 fn curve_cards(&self,point_mode:bool,cx:&mut Context<Self>)->Div{
  let mut grid=div().flex().flex_col().gap_2();
  let count=if point_mode{self.curve_points.len()}else{self.gamma.len()};
  if count==0{return grid;}
  let selected=self.curve_channel.min(count-1);
  grid=grid.child(div().flex().flex_wrap().gap_1().children((0..count).map(|i|seg(("tone-channel",i),format!("{} · {}",i+1,self.ink_name(i)),i==selected).child(swatch(self.ink_color(i),12.)).on_click(cx.listener(move|v,_,_,cx|{v.curve_channel=i;v.curve_drag=None;cx.notify();})))));
  for i in [selected]{
    let name=self.ink_name(i);
    let color=self.ink_color(i);
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
     .on_mouse_up(MouseButton::Left,cx.listener(|v,_,_,_|{v.curve_drag=None;}));
    let mut card=div().flex().flex_col().flex_1().min_w(px(0.)).gap_1().p_2().rounded_md().bg(rgb(0x282828))
     .child(div().flex().items_center().justify_between().child(div().text_sm().child(name))
      .child(seg((if point_mode{"point-reset"}else{"gamma-reset"},i),"Reset",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{if point_mode{v.curve_points[i]=IDENTITY_POINTS[0];}else{v.gamma[i]=1.;}cx.notify();}}))))
     .child(graph)
     .child(div().flex().justify_between().text_xs().text_color(rgb(0xa1a1aa)).child("0%").child(tr("Input →")).child("100%"));
    if point_mode{
     for j in 1..4{card=card.child(div().flex().flex_wrap().gap_1().items_center().text_xs().child(format!("{}% → {:.0}%",j*25,self.curve_points[i][j]*100.))
      .child(seg(("point-minus",i*5+j),"−",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{let p=&mut v.curve_points[i];p[j]=(p[j]-0.01).max(p[j-1]);cx.notify();}})))
      .child(self.numeric_slider(("point-slider",i*5+j),self.curve_points[i][j],self.curve_points[i][j-1],self.curve_points[i][j+1],0.01,move|v,x|v.curve_points[i][j]=x,cx)).child(seg(("point-plus",i*5+j),"+",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{let p=&mut v.curve_points[i];p[j]=(p[j]+0.01).min(p[j+1]);cx.notify();}}))));}
    }else{
     card=card.child(div().flex().flex_wrap().items_center().gap_1().child(seg(("gamma-minus",i),"−",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.gamma[i]=(v.gamma[i]-0.01).max(0.25);cx.notify();}}))).child(format!("γ {:.2}",self.gamma[i])).child(self.numeric_slider(("gamma-slider",i),self.gamma[i],0.25,4.,0.01,move|v,x|v.gamma[i]=x,cx)).child(seg(("gamma-plus",i),"+",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.gamma[i]=(v.gamma[i]+0.01).min(4.);cx.notify();}}))))
      .child(div().text_xs().child(format!("25/50/75% → {:.0}/{:.0}/{:.0}%",100.*0.25_f64.powf(self.gamma[i]),100.*0.5_f64.powf(self.gamma[i]),100.*0.75_f64.powf(self.gamma[i]))));
    }
    grid=grid.child(card);
  }
  grid
 }
 fn curves_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Tone Curves","Drag curve handles or use numeric steps. Curves represent corrections, not measured ink response."));
  if self.gamma.is_empty(){return p.child(tr("Open a supported printer profile first."))}
  let points=self.capability.as_ref().is_some_and(|c|c.reason(Tool::Correction).is_none());
  let point_mode=points&&self.curve_mode==1;
  p=p.child(div().flex().gap_1().flex_wrap().child(seg("tone-gamma","Gamma curves",!point_mode).on_click(cx.listener(|v,_,_,cx|{v.curve_mode=0;v.curve_drag=None;cx.notify();})))
   .when(points,|p|p.child(seg("tone-points","Bounded multipoint curves",point_mode).on_click(cx.listener(|v,_,_,cx|{v.curve_mode=1;v.curve_drag=None;cx.notify();})))));
  if let Some(reason)=self.capability.as_ref().and_then(|c|c.reason(if point_mode{Tool::Correction}else{Tool::Gamma})){return p.child(div().text_color(rgb(0xfbbf24)).child(i18n::diagnostic(reason)));}
  if point_mode{return p.child(self.point_curves_page(cx));}
  p.child(self.curve_cards(false,cx))
   .child(button("reset-all-gamma","Reset all curves").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.gamma.fill(1.);cx.notify();}})))
   .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("ICC v4 mBA gamma edits preserve PCS stages and CLUT data; modified curves are sampled with a 0.5 percentage point error ceiling. Invalid or too-steep curves are refused.")))
   .child(tr("Gamma modifies output separations; A2B stays unchanged. Recheck TAC and channel limits."))
   .child(button("save-curves","Save gamma ICC").bg(rgb(0xc65f24)).on_click(cx.listener(|v,_,_,cx|v.save_edit(0,cx))))
 }
 fn reorder_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Channel Reorder","New channel index → original physical ink."));
  if self.order.is_empty(){return p.child(tr("Open a supported printer profile first."))}
  let unchanged=self.order.iter().enumerate().all(|(i,&j)|i==j);
  p=p.child(div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828))
   .child(div().flex().items_center().justify_between().child(div().text_lg().child(tr("Original channel order"))).child(format!("{} {}",self.order.len(),tr("Channels"))))
   .child(div().flex().flex_wrap().gap_2().children((0..self.order.len()).map(|i|div().flex().flex_col().gap_1().child(swatch(self.ink_color(i),36.)).child(div().text_xs().child(format!("{} · {}",i+1,self.ink_name(i))))))))
   .child(div().flex().items_center().justify_between().gap_2().child(div().text_lg().child(tr("New channel order"))).child(button("reset-order","Reset mapping").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.order=(0..v.order.len()).collect();cx.notify();}}))));
  for (i,&old) in self.order.iter().enumerate(){
   p=p.child(div().flex().flex_wrap().items_center().gap_2().p_3().rounded_md().bg(rgb(if old==i{0x282828}else{0x392c22}))
    .child(div().text_lg().child(format!("{}",i+1))).child(swatch(self.ink_color(old),26.))
    .child(div().flex_1().min_w(px(0.)).flex().flex_col().gap_1().child(self.ink_name(old)).child(div().text_xs().text_color(rgb(0xa1a1aa)).child(format!("{} {}",tr("Original channel"),old+1))))
    .child(seg(("order-up",i),"↑",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy&&i>0{v.order.swap(i,i-1);cx.notify();}})))
    .child(seg(("order-down",i),"↓",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy&&i+1<v.order.len(){v.order.swap(i,i+1);cx.notify();}}))));
  }
  p=p.child(div().text_sm().text_color(rgb(if unchanged{0xa1a1aa}else{0xfb923c})).child(tr(if unchanged{"Original mapping unchanged"}else{"Channel mapping changed"})))
   .child(tr("Changes both A2B input axes and B2A output channels. Unknown vendor tags or calibration data cause rejection. RIP channel order must match the new physical order."))
   .child(tr("Before saving, all four rendering intents are sampled with LittleCMS. Maximum allowed ΔE00, channel deviation and TAC deviation: 0.5 each."));
  p.child(button("save-order","Save reordered ICC").bg(rgb(if unchanged{0x333333}else{0xc65f24})).on_click(cx.listener(|v,_,_,cx|{if v.order.iter().enumerate().any(|(i,&j)|i!=j){v.save_edit(1,cx);}})))
 }
 fn version_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("ICC Version Conversion","Structural conversion of CMYK / nCLR Lab printer profiles, verified in LittleCMS."));
  let Some(profile)=&self.display_profile else{return p.child(tr("Open an ICC/ICM printer profile."))};
  if editing::editable_channel_count(profile).is_err()||profile.pcs()!="Lab "{return p.child(tr("Only simple v2/v4 CMYK / nCLR Lab printer profiles are supported."))}
  let target=if profile.version_major()==2{4}else{2};
  let omitted=versioning::omitted_tags(profile);
  p=p.child(format!("ICC v{} → v{} · {} · {}",profile.version_major(),target,profile.space(),if target==2{"lut16 (mft2)"}else{"mAB / mBA"}))
   .child(div().flex().flex_wrap().gap_2().children((0..profile.channels().len()).map(|j|div().flex().items_center().gap_1().child(swatch(self.ink_color(j),12.)).child(format!("{} · {}",j+1,self.ink_name(j))))))
   .child(tr("Valid colorant names and laydown order tags are retained as original bytes. In ICC v2 these tags may be extensions; verify interpretation in the target RIP."))
   .child(tr("Preserves the six intent CLUTs and adapts the PCS-side Lab encoding; does not regenerate separations or relabel the header only. Unsupported curve/matrix structures fail closed."));
  if !omitted.is_empty(){
   p=p.child(div().text_sm().text_color(rgb(0xfbbf24)).child(format!("Optional / unhandled tags NOT transferred: {}",omitted.join(", "))))
    .child(tr("These may include vendor metadata or gamut data. The original ICC remains intact. Only enable omission for an independent compatibility copy; vendor/RIP behavior is not guaranteed."))
    .child(seg("version-omit",if self.version_allow_omit{"Omit listed tags ✓"}else{"Allow omission of listed tags"},self.version_allow_omit).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.version_allow_omit=!v.version_allow_omit;cx.notify();}})));
  }
  for (i,(label,value)) in [("Max forward ΔE00",self.version_de),("Max channel Δ (percentage points)",self.version_ink)].into_iter().enumerate(){
   let change=move|v:&mut Self,d:f64|{let x=if i==0{&mut v.version_de}else{&mut v.version_ink};*x=(*x+d).clamp(0.05,2.);};
   p=p.child(div().flex().items_center().gap_2().child(div().w(px(220.)).child(tr(label)))
    .child(seg(("version-minus",i),"−",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{change(v,-0.01);cx.notify();}}))).child(format!("{value:.2}"))
    .child(self.numeric_slider(("version-slider",i),value,0.05,2.,0.01,move|v,x|if i==0{v.version_de=x}else{v.version_ink=x},cx)).child(seg(("version-plus",i),"+",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{change(v,0.01);cx.notify();}}))));
  }
  p=p.child(div().flex().gap_1().flex_wrap()
    .child(seg("version-all","Require all intents",!self.version_relative_only).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.version_relative_only=false;v.version_report=None;cx.notify();}})))
    .child(seg("version-rel","Relative / Absolute only",self.version_relative_only).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.version_relative_only=true;v.version_report=None;cx.notify();}}))))
   .when(self.version_relative_only,|p|p.child(div().text_sm().text_color(rgb(0xfbbf24)).child(tr("Compatibility copy: Perceptual and Saturation may change substantially. Do not use those intents in the RIP. All four are measured and reported, but only Relative and Absolute must meet the selected tolerance."))))
   .child(tr("Checks device→PCS color, PCS→device channel differences and TAC for all four intents. Preserves zero-ink channels and bounds; no new ICC saved if required limits fail."))
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
      Ok(r)=>{v.status=format!("Saved ICC v{} ({}): {}",r.target,if r.relative_absolute_only{"Relative/Absolute only"}else{"all intents validated"},path.display());v.version_report=Some(r);v.candidate=Some(path);v.preview=None;v.comparison=None;v.gradient=None;v.gradient_detail=None;v.gradient_error=None;if v.preview_open{v.refresh_gamut(cx);}},
      Err(e)=>v.status=format!("Version conversion refused: {e}")
     }cx.notify();});
    }).detach();
   })));
  if let Some(r)=&self.version_report{
   p=p.child(div().text_lg().child(tr("Validation report")))
    .child(format!("{} device + {} PCS samples per intent",r.forward_samples,r.reverse_samples))
    .child(format!("Channel bound {:?}% · TAC bound {:.3}%",r.bounds.channels.iter().map(|x|100.*x).collect::<Vec<_>>(),100.*r.bounds.tac));
   if !r.colorant_metadata.is_empty(){p=p.child(format!("{}: {}",tr("Retained colorant metadata"),r.colorant_metadata.join(", ")));}
   if r.channels.len()!=4||self.display_profile.as_ref().is_some_and(|v|!v.cmyk()){if !r.colorant_metadata.iter().any(|v|v=="clrt"){p=p.child(div().text_sm().text_color(rgb(0xfbbf24)).child(tr("No valid colorant table was retained; channel names or colorant coordinates may be unavailable in the target RIP.")));}}
   for (j,cap) in r.bounds.channels.iter().enumerate(){p=p.child(div().flex().items_center().gap_2().child(div().w(px(120.)).child(format!("{} · {}",j+1,r.channels[j]))).child(div().flex_1().child(bar(*cap,self.ink_color(j),10.))).child(format!("{:.3}%",cap*100.)));}
   for i in &r.checks{p=p.child(div().text_sm().text_color(rgb(if i.passed{0xa7f3d0}else{0xfbbf24})).child(format!("{} [{}]: mean ΔE00 {:.4} · max {:.4} · channel Δ {:.4} pp · TAC Δ {:.4} pp",i.name,if i.passed{"PASS"}else if i.required{"FAIL"}else{"OUTSIDE TOLERANCE — NOT CERTIFIED"},i.mean_de,i.max_de,i.max_ink_pp,i.max_tac_pp)));}
   if !r.omitted.is_empty(){p=p.child(format!("Omitted by consent: {}",r.omitted.join(", ")));}
  }
  p.child(tr("Sampled equivalence, not a guarantee for all colors or vendor compatibility. Always validate in the target RIP and print-test."))
 }
 fn refresh_gamut(&mut self,cx:&mut Context<Self>){
  if self.busy{return}let Some(reference)=self.source.clone()else{return};
  if let Some(reason)=self.capability.as_ref().and_then(|c|c.reason(Tool::Analysis)){self.preview=None;self.status=reason.to_owned();cx.notify();return}
  let candidate=self.candidate.clone();let grid=self.gamut_grid;
  let reference_intent=self.gamut_reference_intent;let candidate_intent=if self.gamut_separate_intents{self.gamut_candidate_intent}else{reference_intent};
  self.preview=None;self.busy=true;self.status="Sampling effective Lab gamut with LittleCMS…".into();cx.notify();
  let job=self.start_analysis_job(cx);cx.spawn(async move |this,cx|{
   let worker=job.clone();let result=cx.background_spawn(async move{worker.run(||Ok(gamut::overlay_with_intents(&reference,candidate.as_deref(),grid,reference_intent,candidate_intent)))}).await;
   let _=this.update(cx,|view,cx|{if !view.finish_analysis_job(&job,cx){return}
    let result=match result{Ok(r)=>r,Err(e)=>{view.status=e;cx.notify();return}};let status=match &result.reference{Ok(c)=>format!("Gamut ready: {} reference points{}",c.points.len(),match &result.candidate{Some(Ok(c))=>format!(", {} candidate points",c.points.len()),Some(Err(e))=>format!("; candidate failed: {e}"),None=>String::new()}),Err(e)=>format!("Reference gamut failed: {e}")};view.preview=Some(result);view.preview_revision=view.preview_revision.wrapping_add(1);view.busy=false;view.status=status;cx.notify();
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
  let candidate_intent=if self.gamut_separate_intents{self.gamut_candidate_intent}else{self.gamut_reference_intent};
  let name=if self.candidate.is_some(){format!("{}: {} · {} | {}: {} · {}",tr("Reference"),style(0),tr(intent_label(self.gamut_reference_intent)),tr("Candidate"),style(1),tr(intent_label(candidate_intent)))}else{format!("{}: {} · {}",tr("Reference"),style(0),tr(intent_label(self.gamut_reference_intent)))};
  div().flex().flex_col().gap_1().p_2().flex_shrink_0()
   .child(div().flex().items_center().gap_1()
    .child(seg("view-2d","2D",!o.three_d).on_click(cx.listener(|v,_,_,cx|{v.preview_options.three_d=false;cx.notify();})))
    .child(seg("view-3d","3D",o.three_d).on_click(cx.listener(|v,_,_,cx|{v.preview_options.three_d=true;cx.notify();})))
    .child(div().flex_1())
    .child(seg("ref-visible","Ref",o.reference).on_click(cx.listener(|v,_,_,cx|{v.preview_options.reference=!v.preview_options.reference;cx.notify();})))
    .when(self.candidate.is_some(),|p|p.child(seg("cand-visible","Cand",o.candidate).on_click(cx.listener(|v,_,_,cx|{v.preview_options.candidate=!v.preview_options.candidate;cx.notify();})))))
   .child(div().flex().gap_1().flex_wrap().children([("Pts",false,false),(if o.three_d{"Wire"}else{"Outline"},false,true),("Solid",true,false),("Both",true,true)].into_iter().enumerate().map(|(i,(name,solid,wire))|seg(("quick-style",i),name,o.layout==CompareLayout::Shared&&o.solid==solid&&o.wireframe==wire).on_click(cx.listener(move|v,_,_,cx|{v.preview_options.layout=CompareLayout::Shared;v.preview_options.solid=solid;v.preview_options.wireframe=wire;cx.notify();}))))
    .when(self.candidate.is_some(),|p|p.child(seg("mixed-quick","Compare",o.layout!=CompareLayout::Shared).on_click(cx.listener(|v,_,_,cx|{v.preview_options.layout=CompareLayout::ReferenceSolidCandidateWire;cx.notify();})))))
   .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(name))
   .when(!o.three_d,|p|p.child(div().flex().gap_1().items_center().flex_wrap()
    .child(seg("slice-toggle",if o.slice{"Slice"}else{"All L*"},o.slice).on_click(cx.listener(|v,_,_,cx|{v.preview_options.slice=!v.preview_options.slice;cx.notify();})))
    .when(o.slice,|p|p.child(seg("slice-down","−",false).on_click(cx.listener(|v,_,_,cx|{v.preview_options.light=(v.preview_options.light-1.).max(0.);cx.notify();}))).child(format!("L* {:.0}",o.light)).child(self.numeric_slider(("slice-slider",0),o.light as f64,0.,100.,1.,|v,x|v.preview_options.light=x as f32,cx)).child(seg("slice-up","+",false).on_click(cx.listener(|v,_,_,cx|{v.preview_options.light=(v.preview_options.light+1.).min(100.);cx.notify();}))))
    .child(div().text_xs().child(format!("Fill {}%",o.opacity_2d)))
    .child(seg("opacity-down","−",false).on_click(cx.listener(|v,_,_,cx|{v.preview_options.opacity_2d=v.preview_options.opacity_2d.saturating_sub(1);cx.notify();})))
    .child(self.numeric_slider(("opacity-slider",0),o.opacity_2d as f64,0.,100.,1.,|v,x|v.preview_options.opacity_2d=x as _,cx)).child(seg("opacity-up","+",false).on_click(cx.listener(|v,_,_,cx|{v.preview_options.opacity_2d=(v.preview_options.opacity_2d+1).min(100);cx.notify();})))))
 }
 fn preview_panel(&self,width:f32,display:f32,full:bool,cx:&mut Context<Self>)->Div{
  let panel=div().flex().flex_col().w(px(width)).flex_shrink_0().h_full().min_h(px(0.)).overflow_hidden().bg(rgb(0x101010)).when(self.language.rtl(),|p|p.text_right())
   .child(div().flex().items_center().justify_between().p_2().flex_shrink_0()
    .child(div().text_sm().child(tr("Gamut")))
    .child(div().flex().gap_1()
     .child(seg("full-gamut",if full{"Exit expanded"}else{"Expand"},full).on_click(cx.listener(|v,_,_,cx|{v.preview_fullscreen=!v.preview_fullscreen;cx.notify();})))
     .when(!full,|p|p.child(seg("collapse-preview","»",false).on_click(cx.listener(|v,_,_,cx|{v.preview_open=false;cx.notify();}))))))
   .child(self.gamut_toolbar(cx))
   .child(self.gamut_plot(display,"gamut-orbit",cx))
   .child(self.analysis_progress(1,cx))
   .child(div().id("preview-scroll").flex_1().min_h(px(0.)).overflow_y_scroll().p_2().child(self.gamut_controls(cx)));
  panel
 }
 fn gamut_intent_controls(&self,scope:usize,cx:&mut Context<Self>)->Div{
  let intents=[("Perceptual",lcms2::Intent::Perceptual),("Relative",lcms2::Intent::RelativeColorimetric),("Saturation",lcms2::Intent::Saturation),("Absolute",lcms2::Intent::AbsoluteColorimetric)];
  let separate=self.candidate.is_some()&&self.gamut_separate_intents;
  let mut p=div().flex().flex_col().gap_1()
   .child(div().flex().items_center().flex_wrap().gap_1()
    .child(div().text_xs().child(tr(if separate{"Reference intent"}else{"Gamut rendering intent"})))
    .when(self.candidate.is_some(),|p|p.child(seg(("gamut-separate-intents",scope),tr("Separate intents"),separate).on_click(cx.listener(|v,_,_,cx|{
     if !v.busy&&v.candidate.is_some(){v.gamut_separate_intents=!v.gamut_separate_intents;if v.gamut_separate_intents{v.gamut_candidate_intent=v.gamut_reference_intent;}v.preview=None;v.refresh_gamut(cx);cx.notify();}
    })))))
   .child(div().flex().flex_wrap().gap_1().children(intents.into_iter().enumerate().map(|(i,(name,intent))|
    seg(("gamut-reference-intent",scope*4+i),tr(name),self.gamut_reference_intent==intent).on_click(cx.listener(move|v,_,_,cx|{
     if !v.busy&&v.gamut_reference_intent!=intent{v.gamut_reference_intent=intent;v.preview=None;v.refresh_gamut(cx);cx.notify();}
    })))));
  if separate{
   p=p.child(div().text_xs().child(tr("Candidate intent")))
    .child(div().flex().flex_wrap().gap_1().children(intents.into_iter().enumerate().map(|(i,(name,intent))|
     seg(("gamut-candidate-intent",scope*4+i),tr(name),self.gamut_candidate_intent==intent).on_click(cx.listener(move|v,_,_,cx|{
      if !v.busy&&v.candidate.is_some()&&v.gamut_separate_intents&&v.gamut_candidate_intent!=intent{v.gamut_candidate_intent=intent;v.preview=None;v.refresh_gamut(cx);cx.notify();}
     })))));
   if self.gamut_reference_intent!=self.gamut_candidate_intent{
    p=p.child(div().text_xs().text_color(rgb(0xfbbf24)).child(tr("Different intents: coverage changes include rendering intent differences.")));
   }
  }
  p.child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Requested intents apply to both round-trip legs. Native CMM may use standard intent fallback.")))
 }
 fn gamut_controls(&self,cx:&mut Context<Self>)->Div{
  let o=self.preview_options;
  let filename=|p:&Option<PathBuf>|p.as_ref().and_then(|p|p.file_name()).map(|n|n.to_string_lossy().into_owned()).unwrap_or("Not selected".into());
  let mut p=div().flex().flex_col().gap_2().text_sm()
   .child(div().text_xs().child(format!("{}: {}",tr("Reference"),filename(&self.source))))
   .child(div().text_xs().child(format!("{}: {}",tr("Candidate"),filename(&self.candidate))))
   .child(div().flex().gap_1().flex_wrap()
    .child(seg("preview-pick","Compare ICC",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy{if let Some(path)=rfd::FileDialog::new().add_filter("ICC profile",&["icc","icm"]).pick_file(){v.candidate=Some(path);v.preview_options.candidate=true;v.preview=None;v.comparison=None;v.gradient=None;v.gradient_detail=None;v.gradient_error=None;v.refresh_gamut(cx);cx.notify();}}})))
    .child(self.system_dropdown(Picker::Gamut,cx))
    .when(self.candidate.is_some(),|p|p.child(seg("clear-candidate","Clear",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.candidate=None;v.preview=None;v.comparison=None;v.gradient=None;v.gradient_detail=None;v.gradient_error=None;v.refresh_gamut(cx);cx.notify();}}))))
    .child(seg("refresh-gamut",if self.busy{"Working…"}else{"Refresh"},false).on_click(cx.listener(|v,_,_,cx|v.refresh_gamut(cx)))))
   
   .child(self.gamut_intent_controls(0,cx))
   .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Gamut sampling quality")))
    .child(div().flex().flex_wrap().gap_1().children([(9usize,"Fast"),(17,"Balanced"),(33,"High")].into_iter().map(|(grid,label)|seg(("gamut-quality",grid),tr(label),self.gamut_grid==grid).on_click(cx.listener(move|v,_,_,cx|{if !v.busy&&v.gamut_grid!=grid{v.gamut_grid=grid;v.preview=None;v.refresh_gamut(cx);cx.notify();}})))))
    .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Lower sampling quality reduces preview cost, not the output ICC resolution.")))
   .child(div().flex().flex_wrap().gap_1().items_center().child(seg("zoom-out","−",false).on_click(cx.listener(|v,_,_,cx|{v.preview_options.zoom=(v.preview_options.zoom-0.01).max(0.5);cx.notify();}))).child(format!("{:.0}%",100.*o.zoom)).child(self.numeric_slider(("zoom-slider",0),o.zoom as f64,0.5,3.,0.01,|v,x|v.preview_options.zoom=x as f32,cx)).child(seg("zoom-in","+",false).on_click(cx.listener(|v,_,_,cx|{v.preview_options.zoom=(v.preview_options.zoom+0.01).min(3.);cx.notify();}))).child(seg("reset-view","Reset",false).on_click(cx.listener(|v,_,_,cx|{v.preview_options=ViewOptions::default();cx.notify();}))))
   .child(seg("advanced-gamut",if self.gamut_advanced{"Advanced ▴"}else{"Advanced ▾"},self.gamut_advanced).on_click(cx.listener(|v,_,_,cx|{v.gamut_advanced=!v.gamut_advanced;cx.notify();})));
  if let Some(d)=&self.preview{
   if let Err(e)=&d.reference{p=p.child(format!("Reference: {e}"));}
   if let Some(Err(e))=&d.candidate{p=p.child(format!("Candidate: {e}"));}
   let n=usize::from(d.reference.is_ok()&&o.reference)+usize::from(d.candidate.as_ref().is_some_and(|c|c.is_ok())&&o.candidate);
   p=p.child(div().text_xs().text_color(rgb(0xa1a1aa)).child(format!("{n} active profile(s). Floor = one footprint per profile; disconnected regions may have separate outlines.")));
  }
  if self.gamut_advanced{
   p=p.child(div().flex().gap_1().flex_wrap()
    .child(seg("ref-solid","Ref solid",o.layout==CompareLayout::ReferenceSolidCandidateWire).on_click(cx.listener(|v,_,_,cx|{v.preview_options.layout=CompareLayout::ReferenceSolidCandidateWire;cx.notify();})))
    .child(seg("cand-solid","Cand solid",o.layout==CompareLayout::ReferenceWireCandidateSolid).on_click(cx.listener(|v,_,_,cx|{v.preview_options.layout=CompareLayout::ReferenceWireCandidateSolid;cx.notify();}))))
    .child(div().flex().gap_1().flex_wrap()
     .child(seg("profile-colors","Profile colors",o.profile_colors).on_click(cx.listener(|v,_,_,cx|{v.preview_options.profile_colors=true;cx.notify();})))
     .child(seg("compare-tint","Compare tint",!o.profile_colors).on_click(cx.listener(|v,_,_,cx|{v.preview_options.profile_colors=false;cx.notify();}))))
    .child(div().flex().gap_1().flex_wrap()
     .child(seg("floor","Floor",o.floor_contour).on_click(cx.listener(|v,_,_,cx|{v.preview_options.floor_contour=!v.preview_options.floor_contour;cx.notify();}))))
    .child(seg("save-gamut-png","Export PNG",false).on_click(cx.listener(|v,_,_,cx|{if let Some(data)=&v.preview{if let Some(path)=rfd::FileDialog::new().add_filter("PNG",&["png"]).set_file_name("gamut.png").save_file(){let result=color_convert_rust::gamut_raster::render(Some(data),v.preview_options).png().and_then(|bytes|{use std::io::Write;let mut f=tempfile::NamedTempFile::new_in(path.parent().unwrap_or(std::path::Path::new("."))).map_err(|e|e.to_string())?;f.write_all(&bytes).map_err(|e|e.to_string())?;f.persist_noclobber(&path).map_err(|e|e.to_string())?;Ok(())});v.status=result.map(|_|format!("Saved {}",path.display())).unwrap_or_else(|e|format!("Error: {e}"));cx.notify();}}})))
    .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Sampled round-trip ΔE00 ≤ 2 surface; profile colors approximate D50 Lab to sRGB. Wire grid = L* rings and hue meridians. Not a measured physical gamut.")));
  }
  p
 }
 fn analyze_gradient(&mut self,cx:&mut Context<Self>){
  if self.busy{return}let Some(reference)=self.source.clone()else{return};
  let candidate=self.candidate.clone();self.gradient=None;self.gradient_detail=None;self.gradient_selected=0;self.gradient_error=None;self.busy=true;self.status="Analyzing 13 gradients with LittleCMS…".into();cx.notify();
  let job=self.start_analysis_job(cx);cx.spawn(async move |this,cx|{
   let worker=job.clone();let result=cx.background_spawn(async move{worker.run(||gradient::compare(&reference,candidate.as_deref()))}).await;
   let _=this.update(cx,|view,cx|{if !view.finish_analysis_job(&job,cx){return}view.busy=false;match result{Ok(r)=>{view.gradient=Some(r);view.status="Gradient analysis complete.".into();},Err(e)=>{view.gradient_error=Some(e);view.status="Gradient analysis failed.".into();}}cx.notify();});
  }).detach();
 }
 fn gradient_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Gradient Analyzer","13 sRGB ramps × 256 samples, relative colorimetric."))
   .child(div().flex().gap_2().flex_wrap().child(button("run-gradient",if self.busy{"Working…"}else{"Analyze gradients"}).bg(rgb(0xc65f24)).on_click(cx.listener(|v,_,_,cx|v.analyze_gradient(cx)))).child(button("gradient-comparison","Profile Comparison").on_click(cx.listener(|v,_,_,cx|{v.page=Page::Comparison;v.picker=None;cx.notify();}))))
   .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Reference and optional candidate use the same input ramps. Plateau: ΔE00 < 0.05; spike: > 3× neighboring median + 0.5. Heuristics, not measured banding.")));
  if let Some(e)=&self.gradient_error{p=p.child(div().text_color(rgb(0xfca5a5)).child(i18n::diagnostic(e)));}
  if let Some(r)=&self.gradient{
   p=p.child(button("export-gradients","Export gradient CSV").on_click(cx.listener(|v,_,_,cx|{if !v.busy{if let Some(r)=&v.gradient{if let Some(path)=rfd::FileDialog::new().add_filter("CSV",&["csv"]).set_file_name("gradients.csv").save_file(){v.status=reports::gradient(&path,r).map(|_|format!("Saved {}",path.display())).unwrap_or_else(|e|format!("Error: {e}"));cx.notify();}}}})));
   if r.reference.is_empty(){return p;}
   let selected=self.gradient_selected.min(r.reference.len()-1);
   let global=r.reference.iter().chain(r.candidate.iter().flatten()).flat_map(|v|v.steps.iter()).copied().fold(0.1_f64,f64::max);
   let mut overview=div().flex().flex_col().gap_1().p_3().rounded_md().bg(rgb(0x282828))
    .child(div().flex().items_center().justify_between().gap_1().child(div().text_lg().child(tr("Ramp overview"))).child(div().text_xs().child(format!("ΔE00 0–{global:.2}"))))
    .child(div().flex().gap_2().items_center().child(swatch(0x60a5fa,12.)).child(tr("Reference")).when(r.candidate.is_some(),|row|row.child(swatch(0xfb923c,12.)).child(tr("Candidate"))));
   for (i,ramp) in r.reference.iter().enumerate(){
    let candidate=r.candidate.as_ref().and_then(|v|v.get(i));
    let mut lane=div().id(("ramp-select",i)).flex().flex_col().gap_1().p_2().rounded_md().bg(rgb(if i==selected{0x543621}else{0x303030})).hover(|s|s.bg(rgb(0x404040))).cursor_pointer()
     .child(div().flex().items_center().gap_2().child(div().flex_1().min_w(px(0.)).child(ramp.name.clone())).child(format!("{:.3}",ramp.max_step)))
     .child(bar(ramp.max_step/global,0x60a5fa,5.));
    if let Some(b)=candidate{lane=lane.child(div().flex().items_center().gap_2().child(div().flex_1().child(bar(b.max_step/global,0xfb923c,5.))).child(div().text_xs().child(format!("{:.3}",b.max_step))));}
    overview=overview.child(lane.on_click(cx.listener(move|v,_,_,cx|{v.gradient_selected=i;v.gradient_detail=None;cx.notify();})));
   }
   p=p.child(overview).child(div().flex().gap_2().flex_wrap().items_center()
    .child(seg("gradient-shared","Shared scale",self.gradient_shared_scale).on_click(cx.listener(|v,_,_,cx|{v.gradient_shared_scale=true;cx.notify();})))
    .child(seg("gradient-local","Selected ramp scale",!self.gradient_shared_scale).on_click(cx.listener(|v,_,_,cx|{v.gradient_shared_scale=false;cx.notify();}))));
   for (i,a) in r.reference.iter().enumerate().filter(|(i,_)|*i==selected){
    let b=r.candidate.as_ref().and_then(|v|v.get(i));
    let scale=if self.gradient_shared_scale{global}else{a.steps.iter().chain(b.into_iter().flat_map(|v|v.steps.iter())).copied().fold(0.1_f64,f64::max)};
    let mut row=div().flex().flex_col().gap_1().p_3().rounded_md().bg(rgb(0x282828)).child(div().text_lg().child(a.name.clone()))
     .child(format!("Before: max ΔE {:.3} · P95 {:.3} · worst step {}→{} · plateaus {} · spikes {} · max ink step {:.2} pp",a.max_step,a.p95,a.worst_index,a.worst_index+1,a.plateaus,a.spikes,a.max_ink_step));
    if let Some(b)=b{row=row.child(format!("After: max ΔE {:.3} · P95 {:.3} · worst step {}→{} · plateaus {} · spikes {} · max ink step {:.2} pp",b.max_step,b.p95,b.worst_index,b.worst_index+1,b.plateaus,b.spikes,b.max_ink_step));}
    row=row.child(div().flex().gap_2().text_xs().child(div().text_color(rgb(0x60a5fa)).child(tr("Before"))).when(b.is_some(),|p|p.child(div().text_color(rgb(0xfb923c)).child(tr("After")))))
     .child(div().text_sm().child(tr("Gradient step ΔE00")))
     .child(analysis_plot(&a.steps,b.map(|x|x.steps.as_slice()),scale))
     .child(chart_labels(scale))
     .child(div().text_sm().child(tr("Lightness L*")))
     .child(analysis_plot(&a.light,b.map(|x|x.light.as_slice()),100.))
     .child(chart_labels(100.));
    row=row.child(seg(("gradient-inks",i),if self.gradient_detail==Some(i){"Hide ink curves"}else{"Show ink curves"},self.gradient_detail==Some(i)).on_click(cx.listener(move|v,_,_,cx|{v.gradient_detail=if v.gradient_detail==Some(i){None}else{Some(i)};cx.notify();})));
    if self.gradient_detail==Some(i){
     for (side,ramp,channels) in [(0,Some(a),&r.reference_channels),(1,b,&r.candidate_channels)]{
      if let Some(ramp)=ramp{
       row=row.child(div().text_lg().child(tr(if side==0{"Reference"}else{"Candidate"})));
       for (j,ch) in channels.iter().enumerate(){
        let values=&ramp.ink_curves[j];
        row=row.child(div().flex().items_center().gap_2().child(swatch(visible_ink(ch.swatch(self.embedded_names)),16.)).child(color_convert_rust::rtl::visual(ch.label(self.embedded_names))))
         .child(if side==0{analysis_plot(values,None,100.)}else{analysis_plot(&[],Some(values),100.)})
         .child(chart_labels(100.));
       }
      }
     }
    }
    p=p.child(row);
   }
  }
  p
 }
 fn gamut_analysis_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Gamut Viewer","Effective separation coverage and sampled nonconvex surface."))
   .child(self.gamut_intent_controls(1,cx))
   .child(self.gamut_plot(320.,"gamut-page-orbit",cx))
   .child(button("open-gamut-page-preview","Show preview pane").on_click(cx.listener(|this,_,_,cx|{this.preview_open=true;if this.preview.is_none(){this.refresh_gamut(cx);}cx.notify();})))
   .child(div().flex().gap_2().items_center().child(tr("Target tolerance ΔE00")).children([1usize,2,3,5].map(|v|button(("threshold",v),if self.coverage_threshold==v as f64{format!("{v} ✓")}else{v.to_string()}).on_click(cx.listener(move|this,_,_,cx|{this.coverage_threshold=v as f64;cx.notify();})))));
  if let Some(data)=&self.preview{
   if let (Ok(a),Some(Ok(b)))=(&data.reference,&data.candidate){
    if let Some(c)=gamut::coverage(a,b,self.coverage_threshold){
     p=p.child(format!("Matched target grid: {} valid D50 Lab targets",c.total));
     for (label,n,color) in [("Within tolerance in both",c.both,0x60a5fa),("Lost (before only)",c.lost,0xf87171),("Gained (after only)",c.gained,0x34d399),("Outside tolerance in both",c.neither,0xa1a1aa)]{
      let f=n as f32/c.total as f32;p=p.child(div().flex().flex_col().gap_1().child(format!("{label}: {n} ({:.1}%)",100.*f)).child(div().h(px(9.)).bg(rgb(0x3b3b3b)).child(div().w(relative(f)).h_full().bg(rgb(color)))));
     }
     let base=c.both+c.lost;
     if base>0{p=p.child(format!("Lost share of reference-reproduced targets: {:.2}%",100.*c.lost as f64/base as f64));}
    }
    if let (Some(a),Some(b))=(&a.surface,&b.surface){
     p=p.child(format!("Estimated nonconvex volume (Lab³): {:.0} → {:.0} ({:+.2}%)",a.volume,b.volume,if a.volume>0.{100.*(b.volume/a.volume-1.)}else{0.}));
    }
   }else{p=p.child(tr("Select two valid CMYK or nCLR printer profiles to calculate target coverage differences."));}
  }
  p.child(tr("Coverage is a sampled round-trip reproduction test, not proof of physical achievability. Volume estimates integrate sampled cell occupancy at ΔE00 ≤ 2; resolution dependent, not a measured physical gamut."))
 }
 fn correction_options(&self,cx:&mut Context<Self>)->Div{
  let estimate=self.display_profile.as_ref().filter(|p|p.cmyk()).and_then(|p|p.estimated_tac_percent());
  let source_bound=self.display_profile.as_ref().and_then(|p|color_convert_rust::print_space::bounds(p).ok()).map(|b|b.tac*100.);
  let dynamic=self.display_profile.as_ref().is_some_and(|p|!p.cmyk());let maximum=if dynamic{self.gamma.len() as f64*100.}else{400.};
  let mut p=div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828)).child(div().text_lg().child(tr("Output settings")))
   .child(div().flex().flex_wrap().items_center().gap_1().child(tr("CLUT grid")).children([17usize,33,49,65].into_iter().filter(|&g|!dynamic||g<=33).map(|g|button(("edit-grid",g),if self.correction_grid==g{format!("{g} ✓")}else{g.to_string()}).on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.correction_grid=g;cx.notify();}})))))
   .child(div().flex().flex_wrap().items_center().gap_2().child(format!("{}: {:.0}%",tr("TAC ceiling"),self.correction_tac))
    .child(button("edit-tac-minus","−1").on_click(cx.listener(|this,_,_,cx|{if !this.busy{this.correction_tac=(this.correction_tac-1.).max(0.);cx.notify();}})))
    .child(self.numeric_slider(("edit-tac-slider",0),self.correction_tac,0.,maximum,1.,|v,x|v.correction_tac=x,cx)).child(button("edit-tac-plus","+1").on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.correction_tac=(this.correction_tac+1.).min(maximum);cx.notify();}}))));
  if let Some(v)=source_bound{p=p.child(button("edit-tac-bound",format!("{} ({v:.2}%)",tr("Use source TAC upper bound"))).on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.correction_tac=v.clamp(0.,maximum);cx.notify();}})));}
  if let Some(v)=estimate{p=p.child(button("edit-tac-reference",format!("Use reference B2A1 node estimate ({v:.0}%)")).on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.correction_tac=v;cx.notify();}})));}
  p.child(tr("Select a safe explicit TAC. Source channel maxima are enforced automatically. A2B is preserved."))
 }
 fn correction_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Selective Color Correction","Up to six independent soft hue regions in D50 Lab/LCh."));
  if self.display_profile.is_none(){return p.child(tr("Open a supported printer profile first."))}
  if let Some(reason)=self.capability.as_ref().and_then(|c|c.reason(Tool::Correction)){return p.child(div().text_color(rgb(0xfbbf24)).child(i18n::diagnostic(reason)));}
  let selected=self.region_selected;let r=self.regions[selected];
  let source=[65.,45.*r.center.to_radians().cos(),45.*r.center.to_radians().sin()];
  let corrected=color_convert_rust::correction::corrected_regions(source,&self.regions[..self.region_count]);
  let mut visual=div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828))
   .child(div().text_lg().child(tr("Live Lab target preview")))
   .child(div().flex().gap_2().children([(source,"Before"),(corrected,"After")].map(|(lab,label)|div().flex_1().min_w(px(0.)).flex().flex_col().gap_1().child(div().w_full().h(px(64.)).rounded_md().bg(rgb(lab_swatch(lab)))).child(tr(label)).child(div().text_xs().child(format!("L*a*b* {:.1?}",lab))))));
  for after in [false,true]{
   visual=visual.child(div().text_xs().child(tr(if after{"After"}else{"Before"}))).child(div().flex().h(px(20.)).rounded_md().overflow_hidden().children((0..36).map(|i|{
    let angle=(i as f64*10.).to_radians();let lab=[65.,45.*angle.cos(),45.*angle.sin()];
    let out=if after{color_convert_rust::correction::corrected_regions(lab,&self.regions[..self.region_count])}else{lab};
    div().flex_1().h_full().bg(rgb(lab_swatch(out)))
   })));
  }
  p=p.child(visual.child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Illustrative Lab target adjustment, not a measured or predicted printer output."))))
   .child(div().text_lg().child(tr("Hue regions")))
   .child(div().flex().gap_1().flex_wrap().items_center().children((0..self.region_count).map(|i|{
   let r=self.regions[i];let rgb=color_convert_rust::gamut_raster::lab_rgb([65.,(45.*r.center.to_radians().cos()) as f32,(45.*r.center.to_radians().sin()) as f32]);
   let color=(u32::from(rgb[0])<<16)|(u32::from(rgb[1])<<8)|u32::from(rgb[2]);
   seg(("region",i),format!("{} · {:.0}°{}",i+1,r.center,if r.active(){" *"}else{""}),i==selected)
    .child(div().size(px(8.)).rounded_md().bg(gpui::rgb(color)))
    .on_click(cx.listener(move|v,_,_,cx|{v.region_selected=i;cx.notify();}))
  }))
   .when(self.region_count<MAX_REGIONS,|p|p.child(seg("add-region","+ Region",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy&&v.region_count<MAX_REGIONS{v.regions[v.region_count]=Selective{center:(v.regions[v.region_selected].center+60.).rem_euclid(360.),..Default::default()};v.region_selected=v.region_count;v.region_count+=1;cx.notify();}}))))
   .when(self.region_count>1,|p|p.child(seg("remove-region","Remove",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy&&v.region_count>1{let i=v.region_selected;v.regions.copy_within(i+1..v.region_count,i);v.region_count-=1;v.regions[v.region_count]=Selective::default();v.region_selected=i.min(v.region_count-1);cx.notify();}})))));
  p=p.child(div().flex().gap_1().flex_wrap().children([("Red",40.,0xc34c4c),("Yellow",90.,0xdbbb35),("Green",135.,0x47a86a),("Cyan",200.,0x2fa6b8),("Blue",270.,0x547cda),("Magenta",320.,0xb865b5)].into_iter().enumerate().map(|(i,(name,hue,color))|seg(("hue-preset",i),name,false).bg(rgb(color)).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.regions[v.region_selected].center=hue;cx.notify();}})))));
  for (i,(name,value,lo,hi,step,unit)) in [("Hue center",r.center,0.,360.,1.,"°"),("Hue half-width",r.width,10.,180.,1.,"°"),("Lightness",r.light,-30.,30.,1.," L*"),("Chroma",r.chroma_percent,-80.,80.,1.,"%"),("Hue shift",r.hue_shift,-60.,60.,1.,"°")].into_iter().enumerate(){
   let change=move|v:&mut Self,delta:f64|{let r=&mut v.regions[v.region_selected];let f=match i{0=>&mut r.center,1=>&mut r.width,2=>&mut r.light,3=>&mut r.chroma_percent,_=>&mut r.hue_shift};*f=(*f+delta).clamp(lo,hi);};
   p=p.child(div().flex().flex_wrap().items_center().gap_2().p_2().rounded_md().bg(rgb(0x282828)).child(div().w(px(120.)).child(tr(name)))
    .child(seg(("correction-minus",i),"−",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{change(v,-step);cx.notify();}})))
    .child(div().w(px(75.)).child(format!("{value:.0}{unit}")))
    .child(self.numeric_slider(("correction-slider",i),value,lo,hi,1.,move|v,x|{let r=&mut v.regions[v.region_selected];*match i{0=>&mut r.center,1=>&mut r.width,2=>&mut r.light,3=>&mut r.chroma_percent,_=>&mut r.hue_shift}=x;},cx)).child(seg(("correction-plus",i),"+",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{change(v,step);cx.notify();}}))));
  }

  let active=self.regions[..self.region_count].iter().filter(|r|r.active()).count();
  p.child(div().flex().gap_1().child(seg("reset-selective","Reset region",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.regions[v.region_selected]=Selective::default();cx.notify();}})))
   .child(seg("reset-regions","Reset all",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.regions=[Selective::default();MAX_REGIONS];v.region_count=1;v.region_selected=0;cx.notify();}}))))
   .child(format!("{active} active correction region(s). Overlap is normalized; all masks use the original PCS color. Near-neutrals and lightness endpoints remain protected."))
   .child(self.correction_options(cx))
   .child(tr("One resampling pass for all regions, independent B2A intents; preserves A2B. Resampling can change unselected colors. Compare the new ICC before printing."))
   .child(button("save-selective","Generate corrected ICC").on_click(cx.listener(|v,_,_,cx|{if v.regions[..v.region_count].iter().any(|r|r.active()){v.save_edit(3,cx);}else{v.status="Choose a nonzero region adjustment first.".into();cx.notify();}})))
 }
 fn smoothing_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("LUT Smoothing","Bounded six-neighbor averaging in the B2A separation lattice."));
  if self.display_profile.is_none(){return p.child(tr("Open a supported printer profile first."))}
  if let Some(reason)=self.capability.as_ref().and_then(|c|c.reason(Tool::Correction)){return p.child(div().text_color(rgb(0xfbbf24)).child(i18n::diagnostic(reason)));}
  p=p.child(div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828)).child(div().text_lg().child(tr("Smoothing settings"))).child(div().flex().items_center().justify_between().child(tr("Strength")).child(div().text_2xl().child(format!("{:.0}%",self.smoothing*100.)))).child(bar(self.smoothing,0xe87935,12.)).child(div().flex().flex_wrap().items_center().gap_2().child(format!("{}: {:.0}%",tr("Strength"),self.smoothing*100.))
   .child(button("smooth-minus","−1").on_click(cx.listener(|this,_,_,cx|{if !this.busy{this.smoothing=(this.smoothing-0.01).max(0.05);cx.notify();}})))
   .child(self.numeric_slider(("smooth-slider",0),self.smoothing,0.05,0.5,0.01,|v,x|v.smoothing=x,cx)).child(button("smooth-plus","+1").on_click(cx.listener(|this,_,_,cx|{if !this.busy{this.smoothing=(this.smoothing+0.01).min(0.5);cx.notify();}}))))
   .child(div().flex().flex_wrap().gap_1().items_center().child(tr("Passes")).children([1usize,2,3,4,5].map(|n|button(("smooth-pass",n),if n==self.smoothing_passes{format!("{n} ✓")}else{n.to_string()}).on_click(cx.listener(move|this,_,_,cx|{if !this.busy{this.smoothing_passes=n;cx.notify();}}))))) .child(div().text_xs().child(tr("Requested smoothing strength and passes, not measured improvement."))));
  p.child(self.correction_options(cx))
   .child(tr("Lattice boundaries are not averaged; initial resampling and ink constraints can still change them. Use Gradient Analyzer and Profile Comparison after saving to check the trade-off between smoothness and color accuracy."))
   .child(button("save-smoothing","Generate smoothed ICC").bg(rgb(0xc65f24)).on_click(cx.listener(|this,_,_,cx|this.save_edit(4,cx))))
 }
 fn link_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("DeviceLink Generator","CMYK / nCLR → CMYK / nCLR · ICC v4.3"))
   .child(tr("Reference and candidate may have different channel counts. Source and destination ink orders are independent."));
  p=p.child(div().flex().flex_wrap().gap_2().child(div().flex_1().min_w(px(180.)).flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828))
   .child(div().text_sm().text_color(rgb(0x60a5fa)).child(tr("Source profile")))
   .child(self.source.as_ref().map(|p|p.display().to_string()).unwrap_or_else(||tr("Not selected")))
   .child(div().flex().flex_wrap().gap_1().child(button("link-source","Open ICC").on_click(cx.listener(|v,_,_,cx|{v.open(cx);cx.notify();}))))).child(div().flex_1().min_w(px(180.)).flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828))
   .child(div().text_sm().text_color(rgb(0xfb923c)).child(tr("Destination profile")))
   .child(self.candidate.as_ref().map(|p|p.display().to_string()).unwrap_or_else(||tr("Not selected")))
   .child(div().flex().flex_wrap().gap_1().child(button("link-candidate","Select candidate ICC").on_click(cx.listener(|v,_,_,cx|{
    if !v.busy{if let Some(path)=rfd::FileDialog::new().add_filter("ICC profile",&["icc","icm"]).pick_file(){v.candidate=Some(path);v.preview=None;v.comparison=None;v.compare_error=None;v.gradient=None;v.gradient_detail=None;v.gradient_error=None;if v.preview_open{v.refresh_gamut(cx);}cx.notify();}}
   }))).child(self.system_dropdown(Picker::Comparison,cx)))));
  p=p.child(div().flex().flex_wrap().gap_1().items_center().child(tr("Rendering intent"))
   .children([("Relative",lcms2::Intent::RelativeColorimetric),("Perceptual",lcms2::Intent::Perceptual),("Saturation",lcms2::Intent::Saturation),("Absolute",lcms2::Intent::AbsoluteColorimetric)].into_iter().enumerate().map(|(i,(label,intent))|seg(("link-intent",i),label,self.link_intent==intent).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.link_intent=intent;cx.notify();}})))));
  p=p.child(div().flex().flex_wrap().gap_1().items_center().child(tr("CLUT grid"))
   .children([0usize,2,3,4,5,7,9,13,17,25,33].into_iter().map(|g|seg(("link-grid",g),if g==0{"Automatic".to_owned()}else{g.to_string()},self.link_grid==g).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.link_grid=g;cx.notify();}})))))
   .child(tr("Automatic selects the largest supported grid within a 2,000,000-value CLUT budget. High input-channel counts may require a coarse grid."));
  p=p.child(div().flex().flex_wrap().items_center().gap_2().child(format!("{}: {:.1} pp",tr("Maximum channel deviation"),self.link_tolerance))
   .child(seg("link-tol-minus","−",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.link_tolerance=Scale{low:0.1,high:10.,step:0.1}.adjust(v.link_tolerance,-1);cx.notify();}})))
   .child(self.numeric_slider(("link-tolerance",0),self.link_tolerance,0.1,10.,0.1,|v,x|v.link_tolerance=x,cx))
   .child(seg("link-tol-plus","+",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.link_tolerance=Scale{low:0.1,high:10.,step:0.1}.adjust(v.link_tolerance,1);cx.notify();}}))))
   .child(tr("The selected intent is baked into the link. No special black preservation, BPC, or certified physical ink-limit guarantee. Excessive sampled channel error prevents saving."));
  p=p.child(button("save-link",if self.busy{"Working…"}else{"Generate DeviceLink"}).bg(rgb(0xc65f24)).on_click(cx.listener(|v,_,_,cx|{
    if v.busy{return}let (Some(a),Some(b))=(v.source.clone(),v.candidate.clone())else{v.status="Select two printer profiles.".into();cx.notify();return};
    let Some(mut dest)=rfd::FileDialog::new().add_filter("ICC",&["icc"]).set_file_name("print_DeviceLink.icc").save_file()else{return};if dest.extension().is_none(){dest.set_extension("icc");}
    let settings=device_link::LinkSettings{grid:v.link_grid,max_ink_pp:v.link_tolerance,intent:v.link_intent};
    v.busy=true;v.link_report=None;v.status="Generating DeviceLink…".into();cx.notify();
    cx.spawn(async move|this,cx|{
     let aa=a.clone();let bb=b.clone();let dd=dest.clone();
     let result=cx.background_spawn(async move{device_link::device_link_multi(&aa,&bb,&dd,settings)}).await;
     let _=this.update(cx,|v,cx|{v.busy=false;match result{Ok(r)=>{v.status=format!("Saved DeviceLink: {} | {} samples",dest.display(),r.sample_count);v.link_report=Some((a,b,dest,r));},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
    }).detach();
   })));
  if let Some((source,destination,output,r))=&self.link_report{
   let mut report=div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828))
    .child(div().text_lg().child(tr("Last generated DeviceLink")))
    .child(output.display().to_string())
    .child(format!("{} → {}",source.display(),destination.display()))
    .child(format!("{} ({} {}) → {} ({} {})",r.source_space,r.source_channels.len(),tr("Channels"),r.destination_space,r.destination_channels.len(),tr("Channels")))
    .child(format!("{:?} · {}^{} = {} {} · {:.2} MiB",r.intent,r.grid,r.source_channels.len(),r.nodes,tr("CLUT nodes"),r.byte_count as f64/1048576.))
    .child(format!("{}: {}",tr("Validation samples"),r.sample_count))
    .child(format!("{}: {:.3} pp",tr("Maximum channel deviation"),r.max_channel_difference))
    .child(format!("{}: {:.3} pp",tr("Maximum TAC deviation"),r.max_tac_difference))
    .child(format!("{}: {:.3}%",tr("Peak sampled TAC"),r.peak_tac))
    .child(format!("ΔE00 {} {:.3} · P95 {:.3} · {} {:.3}",tr("Mean"),r.mean_de,r.p95_de,tr("Maximum"),r.max_de))
    .child(div().text_sm().child(tr("Source channel order")))
    .child(div().flex().flex_wrap().gap_2().children(r.source_channels.iter().enumerate().map(|(j,ch)|div().flex().items_center().gap_1().child(swatch(visible_ink(ch.swatch(self.embedded_names)),16.)).child(format!("{} · {}",j+1,color_convert_rust::rtl::visual(ch.label(self.embedded_names)))))))
    .child(div().text_sm().child(tr("Destination sampled channel maxima")));
   for (j,ch) in r.destination_channels.iter().enumerate(){
    let name=color_convert_rust::rtl::visual(ch.label(self.embedded_names));let color=visible_ink(ch.swatch(self.embedded_names));
    report=report.child(div().flex().gap_2().items_center().child(swatch(color,16.)).child(div().w(px(115.)).child(format!("{} · {name}",j+1))).child(div().flex_1().child(bar(r.maxima[j]/100.,color,12.))).child(format!("{:.2}%",r.maxima[j])));
   }
   if r.generated_colorant_tables>0{report=report.child(div().text_xs().text_color(rgb(0xfbbf24)).child(tr("Missing or incompatible colorant tables were reconstructed from predicted single-ink Lab values, not measured spectra. Generic channel names are used if none exist.")));}
   p=p.child(report.child(tr("Validation compares the link with the direct profile chain, not measured print color. Sampled results are not exhaustive.")));
  }
  p
 }
 fn optimizer_constraints(&self,caps:&[f64],tac:f64,cx:&mut Context<Self>)->Div{
  let mut weights=div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828)).child(div().flex().items_center().justify_between().gap_1().child(div().text_lg().child(tr("Ink weights and limits"))).child(seg("weights-expand",if self.recipe_weights_open{"Hide weights"}else{"Edit weights"},self.recipe_weights_open).on_click(cx.listener(|v,_,_,cx|{v.recipe_weights_open=!v.recipe_weights_open;cx.notify();})))).child(div().text_xs().child(tr("Relative weight 1 is the baseline; ink ceilings come from Rebuilder.")));
  if self.recipe_weights_open{weights=weights.child(button("reset-weights","Reset weights to 1").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.recipe_settings.weights.fill(1.);v.recipe_result=None;cx.notify();}})));
  }
  for j in 0..self.gamma.len(){
   weights=weights.child(div().flex().flex_col().gap_1().child(div().flex().items_center().gap_2().child(swatch(self.ink_color(j),14.)).child(div().flex_1().child(self.ink_name(j))).child(div().text_xs().child(format!("{} {:.1}% · {} {:.2}",tr("Ink ceiling"),caps[j],tr("Relative weight"),self.recipe_settings.weights[j])))).child(bar(caps[j]/100.,self.ink_color(j),6.)).when(self.recipe_weights_open,|row|row.child(div().flex().flex_wrap().items_center().gap_1()
    .child(button(("weight-minus",j),"−0.01").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.recipe_settings.weights[j]=(v.recipe_settings.weights[j]-0.01).max(0.01);v.recipe_result=None;cx.notify();}})))
    .child(self.numeric_slider(("weight-slider",j),self.recipe_settings.weights[j],0.01,100.,0.01,move|v,x|{v.recipe_settings.weights[j]=x;v.recipe_result=None;},cx)).child(button(("weight-plus",j),"+0.01").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.recipe_settings.weights[j]=(v.recipe_settings.weights[j]+0.01).min(100.);v.recipe_result=None;cx.notify();}}))))));
  }
  weights.child(ink_budget_dynamic(caps,tac)).child(div().flex().flex_wrap().items_center().gap_2().child(format!("{}: {:.1}",tr("ΔE00 tolerance"),self.recipe_settings.tolerance))
   .child(button("recipe-tolerance-minus","−0.1").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.recipe_settings.tolerance=((v.recipe_settings.tolerance*10.).round()-1.).max(1.)/10.;v.recipe_result=None;cx.notify();}})))
   .child(self.numeric_slider(("recipe-tolerance-slider",0usize),self.recipe_settings.tolerance,0.1,5.,0.1,|v,x|{v.recipe_settings.tolerance=x;v.recipe_result=None;},cx))
   .child(button("recipe-tolerance-plus","+0.1").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.recipe_settings.tolerance=((v.recipe_settings.tolerance*10.).round()+1.).min(50.)/10.;v.recipe_result=None;cx.notify();}}))))
 }
 fn recipe_page(&self,cx:&mut Context<Self>)->Div{
  if self.gamma.is_empty()||self.recipe_settings.weights.len()!=self.gamma.len(){return div().child(tr("Open a supported printer profile first."))}
  let dynamic=self.display_profile.as_ref().is_some_and(|p|!p.cmyk());
  let caps=self.caps.clone();let tac=self.tac;
  if caps.len()!=self.gamma.len(){return div().child(tr("Open a supported printer profile first."))}
  let mut p=div().flex().flex_col().gap_3().child(heading("Ink / Cost Optimizer","Find a bounded device recipe for a D50 Lab target."))
   .child(div().flex().gap_1().flex_wrap().children(["Single color recipe","Whole-profile optimization"].into_iter().enumerate().map(|(i,label)|seg(("optimizer-tab",i),label,self.recipe_tab==i).on_click(cx.listener(move|v,_,_,cx|{v.recipe_tab=i;cx.notify();})))));
  if self.recipe_tab==1{
   if let Some(reason)=self.capability.as_ref().and_then(|c|c.reason(Tool::Optimization)){return p.child(div().text_color(rgb(0xfbbf24)).child(i18n::diagnostic(reason)));}
  if !self.display_profile.as_ref().is_some_and(|p|matches!(p.version_major(),2|4)&&p.pcs()=="Lab "&&color_convert_rust::multichannel::channel_count(p).is_ok()){return p.child(tr("Whole-profile optimization requires compatible ICC v2 LUT16 or v4 mBA."))}
  return p.child(div().text_lg().child(tr("Whole-profile ink optimization (v2 LUT16 / v4 mBA)")))
   .child(self.optimizer_constraints(&caps,tac,cx))
   .child(self.correction_options(cx))
   .child(tr("Whole-profile optimization preserves source channel bounds and uses the stricter of the Rebuilder TAC and correction TAC. It refuses an infeasible original separation. Node-level color tolerance does not certify interpolated color or real ink savings."))
   .child(button("optimize-profile","Generate ink-optimized ICC").on_click(cx.listener(|v,_,_,cx|v.save_edit(5,cx))));
  }

  if let Some(reason)=self.capability.as_ref().and_then(|c|c.reason(Tool::Recipe)){return p.child(div().text_color(rgb(0xfbbf24)).child(i18n::diagnostic(reason)));}
   p=p.child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Uses the reference A2B prediction. Relative ink weights are cost proxies, not actual mass or currency. Search is approximate and can fail to find a feasible recipe.")));
  let mut target=div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828)).child(div().flex().items_center().gap_3().child(swatch(lab_swatch(self.recipe_target),64.)).child(div().flex().flex_col().gap_1().child(div().text_lg().child(tr("Target Lab"))).child(format!("{:.1?}",self.recipe_target)).child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Approximate screen color")))));
  for (i,(name,lo,hi)) in [("L*",0.,100.),("a*",-128.,127.),("b*",-128.,127.)].into_iter().enumerate(){
   target=target.child(div().flex().flex_wrap().items_center().gap_1().child(format!("{name}: {:.1}",self.recipe_target[i]))
    .child(button(("recipe-target-minus",i),"−0.1").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.recipe_target[i]=(v.recipe_target[i]-0.1).max(lo);v.recipe_result=None;cx.notify();}})))
    .child(self.numeric_slider(("recipe-target-slider",i),self.recipe_target[i],lo,hi,0.1,move|v,x|{v.recipe_target[i]=x;v.recipe_result=None;},cx)).child(button(("recipe-target-plus",i),"+0.1").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.recipe_target[i]=(v.recipe_target[i]+0.1).min(hi);v.recipe_result=None;cx.notify();}}))));
  }
  p=p.child(target).child(self.optimizer_constraints(&caps,tac,cx))
   .child(button("solve-recipe",if self.busy{"Working…"}else{"Find ink recipes"}).bg(rgb(0xc65f24)).on_click(cx.listener(|v,_,_,cx|{
    if v.busy{return}let Some(p)=v.source.clone()else{return};let target=v.recipe_target;let mut s=v.recipe_settings.clone();let dynamic=v.display_profile.as_ref().is_some_and(|p|!p.cmyk());s.caps=v.caps.clone();s.tac=v.tac;
    v.busy=true;v.recipe_result=None;v.status="Searching feasible ink recipes…".into();cx.notify();
    let job=v.start_analysis_job(cx);cx.spawn(async move|this,cx|{
     let worker=job.clone();let result=cx.background_spawn(async move{worker.run(||optimizer::find_multi(&p,target,s))}).await;
     let _=this.update(cx,|v,cx|{if !v.finish_analysis_job(&job,cx){return}v.busy=false;match result{Ok(r)=>{v.status=format!("{} candidate recipes evaluated",r.evaluated);v.recipe_result=Some(r)},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
    }).detach();
   })));
  if let Some(r)=&self.recipe_result{
   let stale=r.settings.tac!=tac||r.settings.caps!=caps;
   p=p.child(div().flex().items_center().justify_between().gap_2().child(div().text_lg().child(tr("Recipe comparison"))).child(div().text_xs().child(format!("{}: {}",tr("Validation samples"),r.evaluated))));
   if stale{p=p.child(div().text_color(rgb(0xfbbf24)).child(tr("Rebuilder limits changed; run recipe search again.")));}
   if let Some(c)=&r.cheapest{let savings=if r.baseline.cost>0.{100.*(1.-c.cost/r.baseline.cost)}else{0.};p=p.child(div().p_3().rounded_md().bg(rgb(0x282828)).child(div().text_lg().child(format!("{}: {:+.2}%",tr("Relative cost reduction"),savings))).child(format!("TAC {:.1}% → {:.1}%",r.baseline.tac,c.tac)));}
   let recipes=[("Original separation",Some(&r.baseline)),("Closest feasible",Some(&r.closest)),("Lowest cost found within tolerance",r.cheapest.as_ref())];
   let mut cols=div().flex().gap_2();
   for (label,value) in recipes{
    let mut card=div().flex_1().min_w(px(0.)).flex().flex_col().gap_1().p_2().rounded_md().bg(rgb(0x282828)).child(div().text_xs().child(tr(label)));
    if let Some(v)=value{card=card.child(div().w_full().h(px(42.)).rounded_md().bg(rgb(lab_swatch(v.lab)))).child(format!("ΔE00 {:.3}",v.delta_e)).child(format!("TAC {:.1}%",v.tac)).child(format!("{} {:.2}",tr("Relative cost"),v.cost)).child(div().text_xs().child(format!("Lab {:.1?}",v.lab))).when(!v.feasible,|p|p.child(div().text_xs().text_color(rgb(0xfbbf24)).child(tr("Original separation exceeds requested limits."))));}
    else{card=card.child(tr("No solution found within tolerance"));}
    cols=cols.child(card);
   }
   p=p.child(cols).child(div().text_lg().child(tr("Ink recipe by channel")));
   for j in 0..caps.len(){
    let mut row=div().flex().flex_col().gap_1().p_2().rounded_md().bg(rgb(0x282828)).child(div().flex().items_center().gap_1().child(swatch(self.ink_color(j),14.)).child(self.ink_name(j)));
    let mut values=div().flex().gap_2();
    for (_,r) in recipes{
     let mut cell=div().flex_1().min_w(px(0.)).flex().flex_col().gap_1();
     if let Some(v)=r.and_then(|r|r.ink.get(j)){cell=cell.child(format!("{v:.1}%")).child(bar(*v/100.,self.ink_color(j),9.));}else{cell=cell.child("—");}
     values=values.child(cell);
    }
    p=p.child(row.child(values));
   }
  }
  p
 }
 fn ensure_lab_target(&mut self,cx:&mut Context<Self>){
  if self.lab_loading||self.lab_attempted||self.characterization.is_some(){return}
  let Some(source)=self.source.clone()else{return};
  self.lab_attempted=true;self.lab_loading=true;self.lab_error=None;let epoch=self.lab_epoch;cx.notify();
  cx.spawn(async move|this,cx|{
   let result=cx.background_spawn(async move{
    Profile::read(&source).and_then(|p|color_convert_rust::characterization::automatic_chart(&p))
   }).await;
   let _=this.update(cx,|v,cx|{
    if v.lab_epoch!=epoch{return}
    v.lab_loading=false;
    match result{Ok((c,notice))=>{v.characterization_page=c.patches.first().map(|p|p.page).unwrap_or(1);v.characterization_patch=if c.patches.is_empty(){None}else{Some(0)};v.characterization=Some(c);v.lab_error=notice;v.characterization_revision=v.characterization_revision.wrapping_add(1);},Err(e)=>v.lab_error=Some(e)}
    cx.notify();
   });
  }).detach();
 }
 fn characterization_panel(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828))
   .child(div().text_lg().child(tr("Characterization Data Explorer")))
   .child(tr("Recover embedded target coordinates and measured spectra when available. Predicted Lab is a separate ICC model result."));
  if self.source.is_none(){return p.child(tr("Open a printer ICC to preview its patches."));}
  if self.lab_loading{return p.child(div().text_sm().text_color(rgb(0xfb923c)).child(tr("Loading color patches automatically…")));}
  if let Some(e)=&self.lab_error{p=p.child(div().text_xs().text_color(rgb(0xa1a1aa)).child(i18n::diagnostic(e)));}
  let ready=self.characterization.is_some();
  if !ready&&!self.busy{p=p.child(button("retry-auto-target","Retry automatic target").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.lab_attempted=false;v.ensure_lab_target(cx);}}))); }
  if !ready||self.lab_advanced{p=p.child(div().flex().gap_2().flex_wrap().children([false,true].map(|virtual_mode|{
   button(("characterization-load",virtual_mode as usize),if virtual_mode{"Generate virtual target"}else{"Extract embedded target"}).on_click(cx.listener(move|v,_,_,cx|{
    if v.busy||v.lab_loading{return}let Some(source)=v.source.clone()else{return};v.busy=true;v.lab_error=None;v.characterization=None;v.ncolor_report=None;v.ncolor_inverse=None;v.ncolor_audit=None;v.ncolor_ramp=None;v.ncolor_regularized=None;v.ncolor_lattice=None;v.ncolor_forward=None;v.characterization_patch=None;v.characterization_fit=None;v.characterization_measured=false;v.characterization_heat=false;v.characterization_filter=target_tools::Filter::default();v.characterization_cloud=false;v.characterization_revision=v.characterization_revision.wrapping_add(1);v.status="Reading characterization data…".into();cx.notify();
    cx.spawn(async move|this,cx|{
     let result=cx.background_spawn(async move{Profile::read(&source).and_then(|p|if virtual_mode{color_convert_rust::characterization::virtual_chart(&p)}else{color_convert_rust::characterization::extract(&p)})}).await;
     let _=this.update(cx,|v,cx|{v.busy=false;match result{Ok(c)=>{v.characterization_page=c.patches.first().map(|p|p.page).unwrap_or(1);v.characterization=Some(c);v.status="Characterization target ready".into()},Err(e)=>{v.lab_error=Some(e.clone());v.status=e}}cx.notify();});
    }).detach();
   }))
  })));}
  p=p.child(button("target-import-cgats","Import CGATS / TI3").on_click(cx.listener(|v,_,_,cx|{
   if v.busy||v.lab_loading{return}let Some(source)=v.source.clone()else{return};let Some(path)=rfd::FileDialog::new().add_filter("CGATS",&["txt","cgats","ti3"]).pick_file()else{return};
   v.busy=true;v.lab_error=None;v.characterization=None;v.ncolor_report=None;v.ncolor_inverse=None;v.ncolor_audit=None;v.ncolor_ramp=None;v.ncolor_regularized=None;v.ncolor_lattice=None;v.ncolor_forward=None;v.characterization_patch=None;v.characterization_fit=None;v.characterization_measured=false;v.characterization_heat=false;v.characterization_filter=target_tools::Filter::default();v.characterization_cloud=false;v.characterization_revision=v.characterization_revision.wrapping_add(1);v.status="Reading measured CGATS…".into();cx.notify();
   cx.spawn(async move|this,cx|{
    let result=cx.background_spawn(async move{Profile::read(&source).and_then(|p|color_convert_rust::characterization::import_cgats(&p,&path))}).await;
    let _=this.update(cx,|v,cx|{v.busy=false;match result{Ok(c)=>{v.characterization_page=c.patches.first().map(|p|p.page).unwrap_or(1);v.characterization=Some(c);v.status="CGATS target ready".into()},Err(e)=>{v.lab_error=Some(e.clone());v.status=e}}cx.notify();});
   }).detach();
  })));
  let Some(c)=&self.characterization else{return p};
  let pages=c.patches.iter().map(|v|v.page).collect::<std::collections::BTreeSet<_>>();
  p=p.child(format!("{} · {} patches · {} pages · {} spectra",c.source,c.patches.len(),pages.len(),c.patches.iter().filter(|v|v.spectrum.is_some()).count()))
   .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr(if c.original_layout{"Original patch layout"}else{"Reconstructed preview layout"})))
   .child(div().flex().gap_1().flex_wrap()
    .child(seg("target-predicted","ICC predicted",!self.characterization_measured&&!self.characterization_heat).on_click(cx.listener(|v,_,_,cx|{v.characterization_measured=false;v.characterization_heat=false;cx.notify();})))
    .child(seg("target-measured","D50/2° measurement",self.characterization_measured).on_click(cx.listener(|v,_,_,cx|{v.characterization_measured=true;v.characterization_heat=false;cx.notify();})))
    .child(seg("target-heat","ΔE00 heatmap",self.characterization_heat).on_click(cx.listener(|v,_,_,cx|{v.characterization_heat=true;v.characterization_measured=false;cx.notify();}))))
   .child(div().flex().gap_1().flex_wrap().children(pages.into_iter().map(|page|seg(("target-page",page),format!("Page {page}"),self.characterization_page==page).on_click(cx.listener(move|v,_,_,cx|{v.characterization_page=page;v.characterization_patch=None;cx.notify();})))));
  if self.characterization_measured{p=p.child(div().text_xs().child(tr("Measured view uses declared D50/2° Lab or a labelled spectral estimate; missing values appear gray. M0 is not M1.")));}
  if let Some(r)=&self.characterization_fit{p=p.child(format!("{} comparable patches / {} · Mean ΔE00 {:.3} · P95 {:.3} · Max {:.3}",r.count,c.patches.len(),r.mean,r.p95,r.max));}
  p=p.child(div().text_lg().child(tr("Experimental N-color forward model")))
   .child(tr("Compare measured Lab predictions on held-out ink recipes. No ICC is generated."))
   .child(button("ncolor-eval","Evaluate N-color baseline").on_click(cx.listener(|v,_,_,cx|{
    if v.busy||v.lab_loading{return}let Some(c)=v.characterization.clone()else{return};
    let n=v.capability.as_ref().map(|c|c.channels).unwrap_or(0);
    v.busy=true;v.ncolor_report=None;v.status="Evaluating N-color baseline…".into();cx.notify();
    let job=v.start_analysis_job(cx);cx.spawn(async move|this,cx|{
     let worker=job.clone();let result=cx.background_spawn(async move{worker.run(||color_convert_rust::ncolor_model::cross_validate(&c,n))}).await;
     let _=this.update(cx,|v,cx|{if !v.finish_analysis_job(&job,cx){return}v.busy=false;match result{Ok(r)=>{v.ncolor_report=Some(r);v.status="N-color baseline evaluated".into()},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
    }).detach();
   })));
  if let Some(r)=&self.ncolor_report{
   p=p.child(format!("{} · {} · {}: {} · {}: {}",r.channels,r.measurement,tr("Unique recipes"),r.unique_recipes,tr("Duplicate rows"),r.duplicate_rows))
    .child(format!("{} · {}: {} · TAC {:.1}%",tr("Five-fold pooled validation"),tr("Holdout recipes"),r.pooled.test,r.peak_tac_percent));
   let max=r.pooled.nearest.max.max(r.pooled.idw.max).max(r.pooled.local.max).max(0.001);
   for (label,a,b,c) in [("Mean ΔE00",r.pooled.nearest.mean,r.pooled.idw.mean,r.pooled.local.mean),("P95 ΔE00",r.pooled.nearest.p95,r.pooled.idw.p95,r.pooled.local.p95),("Max ΔE00",r.pooled.nearest.max,r.pooled.idw.max,r.pooled.local.max)]{
    p=p.child(div().flex().flex_col().gap_1().p_2().rounded_md().bg(rgb(0x242424)).child(tr(label))
     .child(format!("{}: {:.3}",tr("Nearest neighbor"),a)).child(bar(a/max,0x9ca3af,8.))
     .child(format!("{}: {:.3}",tr("Inverse distance (8 neighbors)"),b)).child(bar(b/max,0x60a5fa,8.))
     .child(format!("{}: {:.3}",tr("Local linear ridge (32 neighbors)"),c)).child(bar(c/max,0xe87935,8.)));
   }
   for (i,f) in r.folds.iter().enumerate(){p=p.child(format!("{} {} · {} / {} · ΔE00 {:.3} / {:.3} / {:.3}",tr("Fold"),i+1,f.train,f.test,f.nearest.mean,f.idw.mean,f.local.mean));}
   p=p.child(format!("{}: {:.3} / {:.3}",tr("Nearest recipe distance (P95 / max)"),r.pooled.nearest_distance.p95,r.pooled.nearest_distance.max))
    .child(div().text_lg().child(tr("Active ink coverage")));
   let max=r.active_counts.iter().copied().max().unwrap_or(1).max(1) as f64;
   for (n,count) in r.active_counts.iter().enumerate(){p=p.child(div().flex().items_center().gap_2().child(div().w(px(72.)).child(format!("{n}: {count}"))).child(div().flex_1().child(bar(*count as f64/max,0xe87935,8.))));}
   p=p.child(div().text_lg().child(tr("Stress test: hold out recipes with at least five active inks")));
   if let Some(v)=&r.stress{p=p.child(format!("{}: {} / {} · ΔE00 Mean/P95/Max",tr("Training / holdout"),v.train,v.test))
    .child(format!("{}: {:.3} / {:.3} / {:.3}",tr("Nearest neighbor"),v.nearest.mean,v.nearest.p95,v.nearest.max))
    .child(format!("{}: {:.3} / {:.3} / {:.3}",tr("Inverse distance (8 neighbors)"),v.idw.mean,v.idw.p95,v.idw.max))
    .child(format!("{}: {:.3} / {:.3} / {:.3}",tr("Local linear ridge (32 neighbors)"),v.local.mean,v.local.p95,v.local.max));}
   else{p=p.child(tr("Not enough recipes for this stress split."));}
   p=p.child(tr("Grouped cross-validation and stress tests are not independent print validation. No ICC is generated."));
  }
  p=p.child(div().text_lg().child(tr("Forward model continuity audit")))
   .child(tr("Compare hard and tapered ridge near reference recipes and in grouped holdouts. Tapered ridge is not used by the inverse solver."))
   .child(button("ncolor-audit","Run continuity audit").on_click(cx.listener(|v,_,_,cx|{
    if v.busy||v.lab_loading{return}let Some(c)=v.characterization.clone()else{return};
    let n=v.capability.as_ref().map(|c|c.channels).unwrap_or(0);
    v.busy=true;v.ncolor_audit=None;v.status="Evaluating model continuity…".into();cx.notify();
    let job=v.start_analysis_job(cx);cx.spawn(async move|this,cx|{
     let worker=job.clone();let result=cx.background_spawn(async move{worker.run(||color_convert_rust::ncolor_model::audit_continuity(&c,n))}).await;
     let _=this.update(cx,|v,cx|{if !v.finish_analysis_job(&job,cx){return}v.busy=false;match result{Ok(r)=>{v.ncolor_audit=Some(r);v.status="Continuity audit completed".into()},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
    }).detach();
   })));
  if let Some(r)=&self.ncolor_audit{
   p=p.child(format!("{}: {} · {}",tr("Reference anchors"),r.anchors,r.channels));
   for t in &r.perturbations{
    p=p.child(div().flex().flex_col().gap_1().p_2().rounded_md().bg(rgb(0x242424))
     .child(format!("±{:.2} pp · {}: {} · ΔE00 Max",t.step_percent,tr("Samples"),t.samples.count))
     .child(format!("{}: {:.4}",tr("Hard ridge"),t.samples.hard.max))
     .child(format!("{}: {:.4}",tr("Tapered ridge candidate"),t.samples.tapered.max)));
   }
   for (label,m) in [("Grouped five-fold holdout",&r.pooled)].into_iter().chain(r.stress.as_ref().map(|m|("High-active-ink stress holdout",m))){
    p=p.child(div().flex().flex_col().gap_1().p_2().rounded_md().bg(rgb(0x242424)).child(tr(label))
     .child(format!("{}: {} · Mean / P95 / Max ΔE00",tr("Samples"),m.count))
     .child(format!("{}: {:.3} / {:.3} / {:.3}",tr("Hard ridge"),m.hard.mean,m.hard.p95,m.hard.max))
     .child(format!("{}: {:.3} / {:.3} / {:.3}",tr("Tapered ridge candidate"),m.tapered.mean,m.tapered.p95,m.tapered.max)));
   }
   p=p.child(tr("Sampled anchor perturbations do not prove global continuity. Compare sparse-region errors before changing the model."));
  }
  p=p.child(div().text_lg().child(tr("Experimental N-color inverse solver")))
   .child(tr("Estimate one ink recipe from supplied Lab characterization. Uses current Rebuilder limits; does not generate an ICC."));
  for (j,(label,lo,hi)) in [("L*",0.,100.),("a*",-128.,127.),("b*",-128.,127.)].into_iter().enumerate(){
   p=p.child(div().flex().items_center().gap_2().child(div().w(px(38.)).child(label))
    .child(div().w(px(54.)).child(format!("{:.1}",self.ncolor_target[j])))
    .child(self.numeric_slider(("ncolor-target",j),self.ncolor_target[j],lo,hi,0.5,move|v,x|{v.ncolor_target[j]=x;v.ncolor_inverse=None;},cx)));
  }
  p=p.child(div().flex().items_center().gap_2().child(tr("Target ΔE00 tolerance"))
   .child(format!("{:.1}",self.ncolor_tolerance)).child(self.numeric_slider(("ncolor-inverse-limit",0),self.ncolor_tolerance,0.1,20.,0.1,|v,x|{v.ncolor_tolerance=x;v.ncolor_inverse=None;},cx)))
   .child(div().flex().items_center().gap_2().child(tr("Maximum recipe distance"))
   .child(format!("{:.2}",self.ncolor_distance)).child(self.numeric_slider(("ncolor-inverse-limit",1),self.ncolor_distance,0.01,1.,0.01,|v,x|{v.ncolor_distance=x;v.ncolor_inverse=None;},cx)))
   .child(format!("{}: {:.1}%",tr("TAC limit"),self.tac))
   .child(div().flex().flex_wrap().gap_1().children(self.caps.iter().enumerate().map(|(j,c)|div().flex().items_center().gap_1().child(swatch(self.ink_color(j),12.)).child(format!("{} ≤ {:.1}%",self.ink_name(j),c)))))
   .child(button("ncolor-inverse-solve","Solve N-color recipe").on_click(cx.listener(|v,_,_,cx|{
    if v.busy||v.lab_loading{return}let Some(c)=v.characterization.clone()else{return};
    let n=v.capability.as_ref().map(|c|c.channels).unwrap_or(0);
    let target=v.ncolor_target;
    let settings=color_convert_rust::ncolor_model::InverseSettings{caps_percent:v.caps.clone(),tac_percent:v.tac,tolerance:v.ncolor_tolerance,max_recipe_distance:v.ncolor_distance};
    v.busy=true;v.ncolor_inverse=None;v.status="Solving N-color recipe…".into();cx.notify();
    let job=v.start_analysis_job(cx);cx.spawn(async move|this,cx|{
     let worker=job.clone();let result=cx.background_spawn(async move{worker.run(||color_convert_rust::ncolor_model::solve_inverse(&c,n,target,settings))}).await;
     let _=this.update(cx,|v,cx|{if !v.finish_analysis_job(&job,cx){return}v.busy=false;match result{Ok(r)=>{v.ncolor_inverse=Some(r);v.status="N-color recipe evaluated".into()},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
    }).detach();
   })));
  if let Some(r)=&self.ncolor_inverse{
   let stale=r.settings.caps_percent!=self.caps||r.settings.tac_percent!=self.tac||r.target!=self.ncolor_target||r.settings.tolerance!=self.ncolor_tolerance||r.settings.max_recipe_distance!=self.ncolor_distance;
   if stale{p=p.child(div().text_color(rgb(0xfbbf24)).child(tr("Settings changed; solve again to update this result.")));}
   p=p.child(div().text_color(rgb(if r.within_tolerance{0x86efac}else{0xfbbf24})).child(tr(if r.within_tolerance{"Predicted target tolerance reached"}else{"Predicted target tolerance NOT reached"})))
    .child(format!("ΔE00 {:.3} → {:.3} · TAC {:.2}% / {:.2}%",r.initial_delta_e,r.delta_e,r.tac_percent,r.settings.tac_percent))
    .child(format!("Lab target {:.2?} · {} {:.2?}",r.target,tr("Predicted Lab"),r.predicted_lab))
    .child(format!("{}: {:.3} / {:.3} · {}: {}",tr("Nearest recipe distance"),r.nearest_recipe_distance,r.settings.max_recipe_distance,tr("Evaluations"),r.evaluations))
    .child(tr(if r.exact_recipe{"Exact recipe in characterization table"}else{"Interpolated recipe — not measured"}));
   for (j,&v) in r.inks_percent.iter().enumerate(){p=p.child(div().flex().items_center().gap_2()
    .child(div().w(px(100.)).child(self.ink_name(j))).child(div().flex_1().child(bar(v/100.,self.ink_color(j),9.)))
    .child(format!("{v:.4}% / {:.4}%",r.settings.caps_percent[j])));}
   p=p.child(format!("{}: {} · {} / {} ",tr("Local ±0.5 pp sensitivity (max ΔE00)"),r.sensitivity_max_de.map(|v|format!("{v:.3}")).unwrap_or_else(||tr("Unavailable")),r.sensitivity_supported,r.sensitivity_unsupported))
    .child(tr("Model-based recipe only. Distance and local sensitivity are diagnostics, not proof of continuity or print accuracy."));
  }
  p=p.child(button("ncolor-ramp","Audit inverse lightness ramp").on_click(cx.listener(|v,_,_,cx|{
    if v.busy||v.lab_loading{return}let Some(c)=v.characterization.clone()else{return};
    let n=v.capability.as_ref().map(|c|c.channels).unwrap_or(0);let center=v.ncolor_target;
    let settings=color_convert_rust::ncolor_model::InverseSettings{caps_percent:v.caps.clone(),tac_percent:v.tac,tolerance:v.ncolor_tolerance,max_recipe_distance:v.ncolor_distance};
    v.busy=true;v.ncolor_ramp=None;v.status="Auditing inverse lightness ramp…".into();cx.notify();
    let job=v.start_analysis_job(cx);cx.spawn(async move|this,cx|{
     let worker=job.clone();let result=cx.background_spawn(async move{worker.run(||color_convert_rust::ncolor_model::audit_inverse_ramp(&c,n,center,settings))}).await;
     let _=this.update(cx,|v,cx|{if !v.finish_analysis_job(&job,cx){return}v.busy=false;match result{Ok(r)=>{v.ncolor_ramp=Some(r);v.status="Inverse ramp audit completed".into()},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
    }).detach();
   })));
  if let Some(r)=&self.ncolor_ramp{
   let stale=r.settings.caps_percent!=self.caps||r.settings.tac_percent!=self.tac||r.center!=self.ncolor_target||r.settings.tolerance!=self.ncolor_tolerance||r.settings.max_recipe_distance!=self.ncolor_distance;
   if stale{p=p.child(div().text_color(rgb(0xfbbf24)).child(tr("Settings changed; solve again to update this result.")));}
   p=p.child(format!("{}: {} / 5 · Max ΔE00 {:.3}",tr("Predicted target tolerance reached"),r.within_tolerance_count,r.max_predicted_error))
    .child(format!("{}: {:.3} pp",tr("Maximum adjacent ink jump"),r.max_channel_jump_pp));
   for (i,q) in r.points.iter().enumerate(){
    p=p.child(div().flex().flex_col().gap_1().p_2().rounded_md().bg(rgb(0x242424))
     .child(format!("L* {:.1} · ΔE00 {:.3} · TAC {:.2}%",q.target[0],q.delta_e,q.tac_percent))
     .child(div().flex().flex_wrap().gap_1().children(q.inks_percent.iter().enumerate().map(|(j,v)|div().flex().items_center().gap_1().child(swatch(self.ink_color(j),10.)).child(format!("{} {:.2}%",self.ink_name(j),v)))))
     .when(i>0,|d|d.child(format!("{}: {:.2} pp · Σ {:.2} pp",tr("Adjacent ink change"),r.adjacent_max_channel_pp[i-1],r.adjacent_total_change_pp[i-1]))));
   }
   p=p.child(tr("Independent adjacent solutions can have large ink changes even when predicted color errors are small. No smoothing or ICC generation is performed."));
  }
  p=p.child(div().text_lg().child(tr("Continuity-regularized inverse ramp")))
   .child(tr("Refine neighboring recipes while retaining ink limits and per-point color tolerance. Experimental five-point ramp only."))
   .child(div().flex().items_center().gap_2().child(tr("Continuity weight")).child(format!("{:.3}",self.ncolor_lambda))
    .child(self.numeric_slider(("ncolor-continuity-weight",0),self.ncolor_lambda,0.,0.1,0.001,|v,x|{v.ncolor_lambda=x;v.ncolor_regularized=None;},cx)))
   .child(button("ncolor-regularized","Compare regularized ramp").on_click(cx.listener(|v,_,_,cx|{
    if v.busy||v.lab_loading{return}let Some(c)=v.characterization.clone()else{return};
    let n=v.capability.as_ref().map(|c|c.channels).unwrap_or(0);let center=v.ncolor_target;let lambda=v.ncolor_lambda;
    let settings=color_convert_rust::ncolor_model::InverseSettings{caps_percent:v.caps.clone(),tac_percent:v.tac,tolerance:v.ncolor_tolerance,max_recipe_distance:v.ncolor_distance};
    v.busy=true;v.ncolor_regularized=None;v.status="Regularizing inverse ramp…".into();cx.notify();
    let job=v.start_analysis_job(cx);cx.spawn(async move|this,cx|{
     let worker=job.clone();let result=cx.background_spawn(async move{worker.run(||color_convert_rust::ncolor_model::regularize_inverse_ramp(&c,n,center,settings,lambda))}).await;
     let _=this.update(cx,|v,cx|{if !v.finish_analysis_job(&job,cx){return}v.busy=false;match result{Ok(r)=>{v.ncolor_regularized=Some(r);v.status="Regularized ramp evaluated".into()},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
    }).detach();
   })));
  if let Some(r)=&self.ncolor_regularized{
   let b=&r.baseline;
   let stale=b.settings.caps_percent!=self.caps||b.settings.tac_percent!=self.tac||b.center!=self.ncolor_target||b.settings.tolerance!=self.ncolor_tolerance||b.settings.max_recipe_distance!=self.ncolor_distance||r.lambda!=self.ncolor_lambda;
   if stale{p=p.child(div().text_color(rgb(0xfbbf24)).child(tr("Settings changed; solve again to update this result.")));}
   p=p.child(format!("{}: {:.3} → {:.3} pp",tr("Maximum adjacent ink jump"),b.max_channel_jump_pp,r.max_channel_jump_pp))
    .child(format!("Max ΔE00 {:.3} → {:.3} · {}/5 → {}/5",b.max_predicted_error,r.max_predicted_error,b.within_tolerance_count,r.within_tolerance_count))
    .child(format!("{}: {:.4} → {:.4} · {}: {}",tr("Continuity objective"),r.initial_objective,r.objective,tr("Refinement evaluations"),r.evaluations));
   for (old,new) in b.points.iter().zip(&r.points){
    let mut section=div().flex().flex_col().gap_1().p_2().rounded_md().bg(rgb(0x242424))
     .child(format!("L* {:.1} · ΔE00 {:.3} → {:.3} · TAC {:.2}% → {:.2}%",new.target[0],old.delta_e,new.delta_e,old.tac_percent,new.tac_percent));
    for (j,(&a,&z)) in old.inks_percent.iter().zip(&new.inks_percent).enumerate(){
     section=section.child(div().flex().items_center().gap_2()
      .child(div().w(px(85.)).child(self.ink_name(j)))
      .child(div().flex_1().flex().flex_col().gap_1().child(bar(a/100.,0x858585,5.)).child(bar(z/100.,self.ink_color(j),7.)))
      .child(format!("{a:.2}% → {z:.2}%")));
    }
    p=p.child(section);
   }
   p=p.child(tr("Lower objective does not guarantee lower maximum jump or global LUT continuity. No ICC is generated."));
  }
  p=p.child(div().text_lg().child(tr("Experimental 3D Lab lattice")))
   .child(tr("27 local Lab nodes and eight cell centers. Uses current target, ink limits and continuity weight. No ICC is generated."));
  for (i,label) in ["L*","a*","b*"].into_iter().enumerate(){
   p=p.child(div().flex().items_center().gap_2().child(format!("{} ± {:.1}",label,self.ncolor_span[i]))
    .child(self.numeric_slider(("ncolor-span",i),self.ncolor_span[i],0.5,25.,0.5,move|v,x|{v.ncolor_span[i]=x;v.ncolor_lattice=None;},cx)));
  }
  p=p.child(button("ncolor-lattice","Audit 3D Lab lattice").on_click(cx.listener(|v,_,_,cx|{
    if v.busy||v.lab_loading{return}let Some(c)=v.characterization.clone()else{return};
    let n=v.capability.as_ref().map(|c|c.channels).unwrap_or(0);
    let inverse=color_convert_rust::ncolor_model::InverseSettings{caps_percent:v.caps.clone(),tac_percent:v.tac,tolerance:v.ncolor_tolerance,max_recipe_distance:v.ncolor_distance};
    let settings=color_convert_rust::ncolor_model::LatticeSettings{center:v.ncolor_target,half_span:v.ncolor_span,inverse,lambda:v.ncolor_lambda};
    v.busy=true;v.ncolor_lattice=None;v.status="Auditing 3D Lab lattice…".into();cx.notify();
    let job=v.start_analysis_job(cx);cx.spawn(async move|this,cx|{
     let worker=job.clone();let result=cx.background_spawn(async move{worker.run(||color_convert_rust::ncolor_model::audit_lab_lattice(&c,n,settings))}).await;
     let _=this.update(cx,|v,cx|{if !v.finish_analysis_job(&job,cx){return}v.busy=false;match result{Ok(r)=>{v.ncolor_lattice=Some(r);v.status="3D lattice audit completed".into()},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
    }).detach();
   })));
  if let Some(r)=&self.ncolor_lattice{
   let s=&r.settings;let stale=s.center!=self.ncolor_target||s.half_span!=self.ncolor_span||s.lambda!=self.ncolor_lambda||s.inverse.caps_percent!=self.caps||s.inverse.tac_percent!=self.tac||s.inverse.tolerance!=self.ncolor_tolerance||s.inverse.max_recipe_distance!=self.ncolor_distance;
   if stale{p=p.child(div().text_color(rgb(0xfbbf24)).child(tr("Settings changed; solve again to update this result.")));}
   let fmt=|v:Option<f64>|v.map(|x|format!("{x:.3}")).unwrap_or_else(||"—".into());
   p=p.child(format!("{}: {} / 27 → {} / 27",tr("Supported lattice nodes"),r.baseline.supported_nodes,r.refined.supported_nodes))
    .child(format!("{}: {} / 27 → {} / 27",tr("Predicted target tolerance reached"),r.baseline.within_tolerance,r.refined.within_tolerance))
    .child(format!("{}: {:.4} → {:.4}",tr("Continuity objective"),r.initial_objective,r.objective))
    .child(format!("{}: {} · {}: {}",tr("Seed evaluations"),r.seed_evaluations,tr("Refinement evaluations"),r.refinement_evaluations));
   for (label,m) in [("Independent lattice",&r.baseline),("Regularized lattice",&r.refined)]{
    let mut section=div().flex().flex_col().gap_1().p_2().rounded_md().bg(rgb(0x242424)).child(tr(label))
     .child(format!("Max TAC {}% · ΔE00 Mean / P95 / Max: {} / {} / {}",fmt(m.max_tac_percent),fmt(m.error.as_ref().map(|e|e.mean)),fmt(m.error.as_ref().map(|e|e.p95)),fmt(m.error.as_ref().map(|e|e.max))))
     .child(format!("{}: {} / 8 · {}: {} / 8 · {} / 8",tr("Complete cells"),m.complete_cells,tr("Supported cell centers"),m.supported_centers,m.centers_within_tolerance))
     .child(format!("{} ΔE00 Mean / P95 / Max: {} / {} / {}",tr("Interpolated centers"),fmt(m.center_error.as_ref().map(|e|e.mean)),fmt(m.center_error.as_ref().map(|e|e.p95)),fmt(m.center_error.as_ref().map(|e|e.max))));
    for (i,axis) in ["L*","a*","b*"].into_iter().enumerate(){section=section.child(format!("{axis} · {}: {} / 18 · {}: {} pp",tr("Supported edges"),m.axis_edges[i],tr("Maximum adjacent ink jump"),fmt(m.axis_max_jump_pp[i])));}
    p=p.child(section);
   }
   p=p.child(div().flex().gap_1().items_center().child(tr("Lightness slice")).children((0..3).map(|i|seg(("ncolor-lattice-slice",i),format!("L* {:.1}",s.center[0]+(i as f64-1.)*s.half_span[0]),self.ncolor_slice==i).on_click(cx.listener(move|v,_,_,cx|{v.ncolor_slice=i;cx.notify();})))));
   for a in 0..3{
    let mut row=div().flex().gap_1();
    for b in 0..3{
     let n=&r.nodes[self.ncolor_slice*9+a*3+b];
     let color=match &n.refined{Some(v) if v.delta_e<=s.inverse.tolerance=>0x245a40,Some(_)=>0x805323,None=>0x383838};
     row=row.child(div().flex_1().p_2().rounded_md().bg(rgb(color)).child(format!("a* {:.1} / b* {:.1}",n.target[1],n.target[2]))
      .child(n.refined.as_ref().map(|v|format!("ΔE00 {:.3}",v.delta_e)).unwrap_or_else(||tr("Unsupported node"))));
    }
    p=p.child(row);
   }
   p=p.child(tr("Missing nodes are never filled with zero ink. Cell-center sampling does not verify every interpolated point or global gamut coverage."));
  }
  p=p.child(div().text_lg().child(tr("Experimental forward CLUT audit")))
   .child(tr("Independent Lab16 multilinear diagnostic with grouped holdout. Full device domain, not current inverse ink limits. No ICC is generated."));
  let n=self.capability.as_ref().map(|c|c.channels).unwrap_or(0);
  let default_grid=if n<=8{3}else{2};
  let grid=if self.ncolor_forward_grid.len()==n{self.ncolor_forward_grid.clone()}else{vec![default_grid;n]};
  p=p.child(div().flex().gap_1().items_center().children([2usize,3,5].map(|g|button(("ncolor-grid-preset",g),format!("{g} × {n}")).on_click(cx.listener(move|v,_,_,cx|{
    if !v.busy{v.ncolor_forward_grid=vec![g;n];v.ncolor_forward=None;cx.notify();}
  })))));
  for (i,&g) in grid.iter().enumerate(){
   p=p.child(div().flex().items_center().gap_2().child(swatch(self.ink_color(i),12.)).child(format!("{}: {}",self.ink_name(i),g))
    .child(self.numeric_slider(("ncolor-forward-grid",i),g as f64,2.,33.,1.,move|v,x|{if v.ncolor_forward_grid.len()!=n{v.ncolor_forward_grid=vec![default_grid;n];}v.ncolor_forward_grid[i]=x.round() as usize;v.ncolor_forward=None;},cx)));
  }
  match color_convert_rust::ncolor_model::forward_grid_plan(&grid){
   Ok(plan)=>{
    p=p.child(format!("{}: {} · {}: {} bytes",tr("Grid nodes"),plan.nodes,tr("Raw Lab16 table size"),plan.raw_bytes))
     .child(tr(if plan.uniform_mft2{"Uniform grid dimensions"}else{"Non-uniform grid dimensions"}))
     .child(button("ncolor-forward-audit","Audit forward CLUT").on_click(cx.listener(move|v,_,_,cx|{
      if v.busy||v.lab_loading{return}let Some(c)=v.characterization.clone()else{return};
      let grid=if v.ncolor_forward_grid.len()==n{v.ncolor_forward_grid.clone()}else{vec![default_grid;n]};
      let distance=v.ncolor_distance;
      v.busy=true;v.ncolor_forward=None;v.status="Auditing forward CLUT…".into();cx.notify();
      let job=v.start_analysis_job(cx);cx.spawn(async move|this,cx|{
       let worker=job.clone();let result=cx.background_spawn(async move{worker.run(||color_convert_rust::ncolor_model::audit_forward_grid(&c,&grid,0,distance))}).await;
       let _=this.update(cx,|v,cx|{if !v.finish_analysis_job(&job,cx){return}v.busy=false;match result{Ok(r)=>{v.ncolor_forward=Some(r);v.status="Forward CLUT audit completed".into()},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
      }).detach();
     })));
   }
   Err(e)=>{p=p.child(div().text_color(rgb(0xfbbf24)).child(e));}
  }
  if let Some(r)=&self.ncolor_forward{
   if r.plan.grid!=grid||r.max_distance!=self.ncolor_distance{p=p.child(div().text_color(rgb(0xfbbf24)).child(tr("Settings changed; solve again to update this result.")));}
   p=p.child(format!("{} · {} {} / {} {}",r.measurement,r.training,tr("Training recipes"),r.holdout,tr("Holdout recipes")));
   for (label,num,den,color) in [("Supported grid nodes",r.supported_nodes,r.plan.nodes,0xe87935),("Direct model coverage",r.direct_supported,r.holdout,0x60a5fa),("Interpolated holdout coverage",r.table_supported,r.holdout,0x86efac)]{
    p=p.child(format!("{}: {num} / {den}",tr(label))).child(bar(num as f64/den.max(1) as f64,color,8.));
   }
   for (label,e) in [("Direct model error",&r.direct_error),("CLUT holdout error",&r.table_error),("Direct model on common holdout",&r.direct_common_error),("Interpolation vs direct model",&r.interpolation_error)]{
    p=p.child(format!("{} · Mean / P95 / Max ΔE00: {}",tr(label),e.as_ref().map(|e|format!("{:.3} / {:.3} / {:.3}",e.mean,e.p95,e.max)).unwrap_or_else(||"—".into())));
   }
   p=p.child(format!("{}: {:.5} · {}: {}",tr("Max Lab16 quantization ΔE00"),r.max_node_quantization_de,tr("Estimated distance operations"),r.estimated_distance_ops))
    .child(tr("Unsupported corners are never filled. Diagnostic interpolation is not native CMM interpolation or ICC PCS normalization."));
  }
  let filter=self.characterization_filter;
  let rows=c.patches.iter().enumerate().filter(|(_,v)|v.page==self.characterization_page).collect::<Vec<_>>();
  let min_row=rows.iter().map(|(_,v)|v.row).min().unwrap_or(0);let min_col=rows.iter().map(|(_,v)|v.column).min().unwrap_or(0);
  let max_row=rows.iter().map(|(_,v)|v.row).max().unwrap_or(0);let max_col=rows.iter().map(|(_,v)|v.column).max().unwrap_or(0);
  if max_row-min_row<100&&max_col-min_col<100{
   let cell=(300./(max_row-min_row+1) as f32).clamp(5.,22.);
   let lookup=rows.iter().map(|(i,p)|((p.row,p.column),*i)).collect::<std::collections::BTreeMap<_,_>>();
   let mut chart=div().flex().flex_col().gap(px(2.)).p_2().rounded_md().bg(rgb(0x191919));
   for row in min_row..=max_row{let mut line=div().flex().gap(px(2.));
    for col in min_col..=max_col{
     if let Some(&i)=lookup.get(&(row,col)){let patch=&c.patches[i];
      line=line.child(div().id(("target-patch",i)).flex_1().h(px(cell)).min_w(px(2.)).bg(rgb(if !filter.matches(patch){0x353535}else if self.characterization_heat{target_tools::error_color(patch.delta_e)}else if self.characterization_measured{patch.comparison_lab.map(lab_swatch).unwrap_or(0x555555)}else{lab_swatch(patch.predicted_lab)})).border_1().border_color(rgb(if self.characterization_patch==Some(i){0xe87935}else{0x303030})).cursor_pointer().on_click(cx.listener(move|v,_,_,cx|{v.characterization_patch=Some(i);cx.notify();})));
     }else{line=line.child(div().flex_1().h(px(cell)));}
    }
    chart=chart.child(line);
   }
   p=p.child(chart);
  }else{p=p.child(tr("Chart dimensions exceed interactive layout limit; export the original records as CSV."));}
  p=p.child(div().flex().items_center().justify_between().flex_wrap().gap_1()
   .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Select a patch to inspect its color and ink recipe.")))
   .child(seg("lab-advanced",if self.lab_advanced{"Hide advanced controls"}else{"Advanced filters and analysis"},self.lab_advanced).on_click(cx.listener(|v,_,_,cx|{v.lab_advanced=!v.lab_advanced;cx.notify();}))));
  if !self.lab_advanced&&(filter.channel.is_some()||filter.delta_min.is_some()||filter.tac_max<maximum_for_chart(c)){p=p.child(div().text_xs().text_color(rgb(0xfbbf24)).child(tr("Advanced filters are active.")));}
  if self.lab_advanced{
   p=p   .child(tr("Measured view uses declared D50/2° Lab or a labelled spectral estimate; missing values appear gray. M0 is not M1."))
   .child(tr("Confirm only if measurement mode, reference white, backing and print condition are comparable. This is fit to characterization data, not independent validation."))
   .child(button("target-compare-absolute","Confirm comparable conditions and calculate ΔE00").on_click(cx.listener(|v,_,_,cx|{
    if v.busy{return}
    if let Some(c)=&mut v.characterization{
     match color_convert_rust::characterization::compare_absolute(c){Ok(r)=>{v.characterization_fit=Some(r);v.characterization_revision=v.characterization_revision.wrapping_add(1);v.status="Characterization fit calculated".into()},Err(e)=>{v.characterization_fit=None;v.status=e}}
     cx.notify();
    }
   })));
  p=p.child(div().text_lg().child(tr("Patch filters")))
   .child(div().flex().flex_wrap().gap_1()
    .child(seg("target-all-ch",tr("All channels"),filter.channel.is_none()).on_click(cx.listener(|v,_,_,cx|{v.characterization_filter.channel=None;v.characterization_revision=v.characterization_revision.wrapping_add(1);cx.notify();})))
    .children((0..c.patches.first().map(|p|p.inks.len()).unwrap_or(0)).map(|j|seg(("target-filter-ch",j),self.ink_name(j),filter.channel==Some(j)).on_click(cx.listener(move|v,_,_,cx|{v.characterization_filter.channel=Some(j);v.characterization_revision=v.characterization_revision.wrapping_add(1);cx.notify();})))));
  if filter.channel.is_some(){
   p=p.child(div().flex().items_center().gap_2().child(format!("Minimum ink {:.0}%",filter.minimum)).child(self.numeric_slider(("target-min",0),filter.minimum,0.,100.,1.,|v,x|{v.characterization_filter.minimum=x.min(v.characterization_filter.maximum);v.characterization_revision=v.characterization_revision.wrapping_add(1);},cx)))
    .child(div().flex().items_center().gap_2().child(format!("Maximum ink {:.0}%",filter.maximum)).child(self.numeric_slider(("target-max",0),filter.maximum,0.,100.,1.,|v,x|{v.characterization_filter.maximum=x.max(v.characterization_filter.minimum);v.characterization_revision=v.characterization_revision.wrapping_add(1);},cx)));
  }
  let maximum=c.patches.first().map(|p|p.inks.len() as f64*100.).unwrap_or(400.);
  p=p.child(div().flex().items_center().gap_2().child(format!("TAC ≤ {:.0}%",filter.tac_max.min(maximum))).child(self.numeric_slider(("target-tac",0),filter.tac_max.min(maximum),0.,maximum,5.,|v,x|{v.characterization_filter.tac_max=x;v.characterization_revision=v.characterization_revision.wrapping_add(1);},cx)))
   .child(div().flex().flex_wrap().gap_1().children([None,Some(1.),Some(2.),Some(5.)].into_iter().enumerate().map(|(i,min)|seg(("target-de-filter",i),min.map(|d|format!("ΔE ≥ {d}")).unwrap_or("Any error".into()),filter.delta_min==min).on_click(cx.listener(move|v,_,_,cx|{v.characterization_filter.delta_min=min;v.characterization_revision=v.characterization_revision.wrapping_add(1);cx.notify();})))))
   .child(button("target-reset-filters",tr("Reset filters")).on_click(cx.listener(|v,_,_,cx|{v.characterization_filter=target_tools::Filter::default();v.characterization_revision=v.characterization_revision.wrapping_add(1);cx.notify();})))
   .child(format!("{} / {} patches match filters · exports retain the full table/page",c.patches.iter().filter(|p|filter.matches(p)).count(),c.patches.len()));
  if self.characterization_heat{
   p=p.child(div().flex().flex_wrap().gap_2().children([(0x278c69,"≤1"),(0x84a94f,"1–2"),(0xe2aa39,"2–5"),(0xd45545,">5"),(0x555555,"No ΔE")].into_iter().map(|(color,label)|div().flex().items_center().gap_1().child(swatch(color,12.)).child(label))));
  }
  }
  if let Some(patch)=self.characterization_patch.and_then(|i|c.patches.get(i)){
   p=p.child(div().flex().items_center().gap_2().child(swatch(lab_swatch(patch.predicted_lab),32.)).child(format!("{} · Page {} · Row {} · Column {}",patch.id,patch.page,patch.row,patch.column)))
    .child(format!("Relative predicted Lab {:.3?} · TAC {:.2}%",patch.predicted_lab,patch.inks.iter().sum::<f64>()));
   if let Some(a)=patch.absolute_lab{p=p.child(format!("Absolute predicted Lab {:.3?}",a));}
   if let Some(l)=patch.measured_lab{p=p.child(format!("Stored measured Lab {:.3?}",l));}
   if let Some(l)=patch.comparison_lab{p=p.child(div().flex().items_center().gap_2().child(swatch(lab_swatch(l),32.)).child(format!("Comparison Lab {:.3?} · {}",l,patch.measurement_basis)));}
   if let Some(d)=patch.delta_e{p=p.child(format!("ΔE00 {:.3}",d));}
   for (j,v) in patch.inks.iter().enumerate(){p=p.child(div().flex().items_center().gap_2().child(div().w(px(100.)).child(self.ink_name(j))).child(div().flex_1().child(bar(*v/100.,self.ink_color(j),10.))).child(format!("{v:.2}%")));}
   if let Some(s)=&patch.spectrum{
    p=p.child(div().text_lg().child(tr("Spectral viewing conditions")))
     .child(div().flex().gap_1().children(Illuminant::ALL.into_iter().enumerate().map(|(i,ill)|seg(("spectral-ill",i),ill.label(),self.spectral_illuminant==ill).on_click(cx.listener(move|v,_,_,cx|{v.spectral_illuminant=ill;cx.notify();})))))
     .child(div().flex().gap_1().children(Observer::ALL.into_iter().enumerate().map(|(i,obs)|seg(("spectral-obs",i),obs.label(),self.spectral_observer==obs).on_click(cx.listener(move|v,_,_,cx|{v.spectral_observer=obs;cx.notify();})))))
     .child(tr("Alternate viewing conditions are spectral estimates only. ICC fit and heatmap remain D50/2°."));
    if let (Some(start),Some(step))=(patch.start_nm,patch.step_nm){
     match cie_spectral::calculate(s,start,step,self.spectral_illuminant,self.spectral_observer){
      Ok(r)=>{p=p.child(format!("{} / {} · Lab {:.4?} · XYZ {:.4?} · White {:.4?}",self.spectral_illuminant.label(),self.spectral_observer.label(),r.lab,r.xyz,r.white))
       .child(tr(if r.endpoint_extended{"Unmeasured wavelength endpoints extended; no bandpass correction."}else{"Measured wavelength coverage; no bandpass correction."}));},
      Err(e)=>{p=p.child(div().text_xs().text_color(rgb(0xfbbf24)).child(e));}
     }
    }
    p=p.child(button("cie-attribution","CIE data / CC BY-SA 4.0").on_click(cx.listener(|_,_,_,cx|cx.open_url("https://www.cie.co.at/data-tables"))));
    let peak=s.iter().copied().fold(1f64,f64::max);
    p=p.child(format!("{} · {} spectral samples · {:?} nm / {:?} nm",patch.measurement,s.len(),patch.start_nm,patch.step_nm))
     .child(div().flex().items_end().gap_1().h(px(80.)).children(s.iter().map(|v|div().flex_1().h(px((75.*v/peak) as f32)).bg(rgb(0xe87935)))));
   }
  }
  if self.lab_advanced{
  p=p.child(seg("target-cloud-toggle",tr("Paired 3D Lab cloud"),self.characterization_cloud).on_click(cx.listener(|v,_,_,cx|{v.characterization_cloud=!v.characterization_cloud;cx.notify();})));
  if self.characterization_cloud{
   let overlay=target_tools::paired_cloud(c,filter);
   p=p.child(tr("Matched target patches: measured D50/2° estimates or declared Lab (blue) and absolute ICC predictions (amber). Not a full gamut boundary."))
    .child(div().flex().gap_2().items_center().child(swatch(0x60a5fa,12.)).child(tr("Measurement")).child(swatch(0xfbbf24,12.)).child(tr("ICC prediction")))
    .child(div().flex().items_center().gap_2().child(tr("Yaw")).child(self.numeric_slider(("target-yaw",0),self.characterization_view.yaw as f64,-3.14,3.14,0.05,|v,x|v.characterization_view.yaw=x as f32,cx)))
    .child(div().flex().items_center().gap_2().child(tr("Pitch")).child(self.numeric_slider(("target-pitch",0),self.characterization_view.pitch as f64,-1.55,1.55,0.05,|v,x|v.characterization_view.pitch=x as f32,cx)))
    .child(div().flex().gap_1()
     .child(seg("target-cloud-ref","Measurement",self.characterization_view.reference).on_click(cx.listener(|v,_,_,cx|{v.characterization_view.reference=!v.characterization_view.reference;cx.notify();})))
     .child(seg("target-cloud-icc","ICC",self.characterization_view.candidate).on_click(cx.listener(|v,_,_,cx|{v.characterization_view.candidate=!v.characterization_view.candidate;cx.notify();}))))
    .child(gamut_view::plot(Some(&overlay),self.characterization_revision,self.characterization_view,&mut self.characterization_cache.borrow_mut(),420.));
  }
  }
  p=p.child(button("target-cgats-export",tr("Export target CGATS")).on_click(cx.listener(|v,_,_,cx|{
   if v.busy{return}let (Some(c),Some(profile))=(&v.characterization,&v.display_profile)else{return};
   if let Some(path)=rfd::FileDialog::new().add_filter("CGATS",&["cgats"]).set_file_name("characterization.cgats").save_file(){
    v.status=match target_tools::export_cgats(c,&path,profile.cmyk()){Ok(())=>format!("Saved {}",path.display()),Err(e)=>e};cx.notify();
   }
  })));
  if let Some(profile)=&self.display_profile{
   let n=c.patches.first().map(|p|p.inks.len()).unwrap_or(0);
   let (caps,tac)=(self.caps.clone(),self.tac);
   if self.lab_advanced{p=p.child(tr("Separated TIFF: 8 mm patches, 2 mm gaps, selected DPI and Rebuilder ink limits. Original physical size is unknown. Check RIP channel mapping; no ICC conversion."))
    .child(format!("{} channels · TAC {:.0}% · {} DPI",n,tac,self.chart_dpi));
   for (j,cap) in caps.iter().enumerate(){p=p.child(div().flex().items_center().gap_2().child(swatch(self.ink_color(j),12.)).child(self.ink_name(j)).child(format!("{cap:.1}%")));}}
   if caps.len()!=n{p=p.child(tr("Configure ink limits for every channel before TIFF export."));}
   else{p=p.child(button("target-tiff-export",tr("Confirm limits and export separated 16-bit TIFF")).on_click(cx.listener(|v,_,_,cx|{
     if v.busy{return}let (Some(c),Some(profile))=(&v.characterization,&v.display_profile)else{return};
     let Some(path)=rfd::FileDialog::new().add_filter("TIFF",&["tif"]).set_file_name("characterization_page.tif").save_file()else{return};
     let (caps,tac)=(v.caps.clone(),v.tac);
     let (c,profile,page,dpi)=(c.clone(),profile.clone(),v.characterization_page,v.chart_dpi);
     v.busy=true;v.status="Exporting device recipes…".into();cx.notify();
     cx.spawn(async move|this,cx|{
      let result=cx.background_spawn(async move{target_tools::export_page_tiff(&c,&profile,page,&path,dpi,caps,tac).map(|_|path)}).await;
      let _=this.update(cx,|v,cx|{v.busy=false;v.status=result.map(|p|format!("Saved {}",p.display())).unwrap_or_else(|e|e);cx.notify();});
     }).detach();
    })));}
  }
  p.child(button("target-export","Export target CSV").on_click(cx.listener(|v,_,_,cx|{
   if v.busy{return}let Some(c)=&v.characterization else{return};
   if let Some(path)=rfd::FileDialog::new().add_filter("CSV",&["csv"]).set_file_name("characterization.csv").save_file(){
    v.status=match color_convert_rust::characterization::export_csv(c,&path){Ok(())=>format!("Saved {}",path.display()),Err(e)=>e};cx.notify();
   }
  })))
 }
 fn lab_page(&self,cx:&mut Context<Self>)->Div{
  let mut p=div().flex().flex_col().gap_3().child(heading("Profiling Lab","Inspect patches, prepare print targets, compare measurements and build CMYK profiles."))
   .child(div().flex().flex_wrap().gap_1().children(["Patch explorer","Print target","Measure & compare","Build ICC"].into_iter().enumerate().map(|(i,label)|seg(("lab-tab",i),label,self.lab_tab==i).on_click(cx.listener(move|v,_,_,cx|{v.lab_tab=i;if i==0{v.ensure_lab_target(cx);}cx.notify();})))));
  if self.lab_tab==0{return p.child(self.characterization_panel(cx));}
  if self.display_profile.is_none(){return p.child(tr("Open a supported printer profile first."));}
  let dynamic=self.display_profile.as_ref().is_some_and(|p|!p.cmyk());
  let (caps,tac)=(self.caps.clone(),self.tac);
  if self.lab_tab==1{
   if let Some(reason)=self.capability.as_ref().and_then(|c|c.reason(Tool::Target)){return p.child(div().text_color(rgb(0xfbbf24)).child(i18n::diagnostic(reason)));}
   let count=color_convert_rust::profiling_target::recipes(&caps,tac,self.lab_levels).map(|v|v.len()).unwrap_or(0);
   return p   .child(tr("Record device, substrate, ink configuration and measurement conditions externally. Use identical geometry and D50 Lab for both charts."))
   .child(div().text_lg().child(tr("Generate device test chart")))
   .child(div().flex().gap_1().items_center().child(tr("Levels per ink")).children([3usize,5,7].map(|n|button(("chart-level",n),if self.lab_levels==n{format!("{n} ✓")}else{n.to_string()}).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.lab_levels=n;cx.notify();}})))))
   .child(format!("{} channels · {} unique patches · caps {:?} · TAC {:.0}% · 8 mm patches, 2 mm gaps",caps.len(),count,caps,tac))
   .child(tr("Bounded design: single ramps, pair mixtures and distributed samples. Not a full factorial or a certified profiling target."))
   .child(div().flex().gap_2().items_center().child(format!("{}: {} DPI",tr("Chart resolution"),self.chart_dpi))
    .child(button("chart-dpi-minus","−10").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.chart_dpi=v.chart_dpi.saturating_sub(10).max(50);cx.notify();}})))
    .child(self.numeric_slider(("dpi-slider",0),self.chart_dpi as f64,50.,1200.,10.,|v,x|v.chart_dpi=x as u32,cx)).child(button("chart-dpi-plus","+10").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.chart_dpi=(v.chart_dpi+10).min(1200);cx.notify();}}))))
   .child(button("generate-chart","Export TIFF chart and measurement CSV").on_click(cx.listener(|v,_,_,cx|{
    if v.busy{return}let Some(p)=v.source.clone()else{return};let Some(root)=rfd::FileDialog::new().pick_folder()else{return};
    let dynamic=v.display_profile.as_ref().is_some_and(|p|!p.cmyk());
    let (caps,tac)=(v.caps.clone(),v.tac);
    let (levels,dpi)=(v.lab_levels,v.chart_dpi);
    v.busy=true;v.status="Generating device chart…".into();cx.notify();
    cx.spawn(async move|this,cx|{
     let result=cx.background_spawn(async move{color_convert_rust::profiling_target::export(&p,&root,levels,&caps,tac,dpi).map(|p|format!("Chart: {}",p.display()))}).await;
     let _=this.update(cx,|v,cx|{v.busy=false;v.status=result.unwrap_or_else(|e|format!("Error: {e}"));cx.notify();});
    }).detach();
   })))
   .child(tr("Print the chart with no RIP color conversion; verify safe ink limits first. Fill blank measured L,a,b after printing; blank values are rejected."));
  }
  if self.lab_tab==2{
  p=p.child(div().text_lg().child(tr("Measured condition comparison")));
  for i in 0..2{p=p.child(button(("measured-file",i),if i==0{"Select measured baseline CSV"}else{"Select measured changed-condition CSV"}).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{if let Some(p)=rfd::FileDialog::new().add_filter("CSV",&["csv"]).pick_file(){v.measured[i]=Some(p);v.measurement_result=None;cx.notify();}}}))).child(self.measured[i].as_ref().map(|p|p.display().to_string()).unwrap_or("Not selected".into()));}
  p=p.child(button("compare-measured","Confirm comparable D50/2° conditions and compare").on_click(cx.listener(|v,_,_,cx|{
   if v.busy{return}let [Some(a),Some(b)]=v.measured.clone()else{return};v.busy=true;v.measurement_result=None;v.status="Comparing actual measurements…".into();cx.notify();
   cx.spawn(async move|this,cx|{
    let result=cx.background_spawn(async move{color_convert_rust::profiling_target::compare(&a,&b)}).await;
    let _=this.update(cx,|v,cx|{v.busy=false;match result{Ok(r)=>{v.measurement_result=Some(r);v.status="Measured comparison finished".into()},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
   }).detach();
  })));
  if let Some(r)=&self.measurement_result{p=p.child(format!("{} patches | Mean ΔE {:.3} | P95 {:.3} | Max {:.3} ({}) | Mean ΔLab (after−before) {:.3?}",r.count,r.mean,r.p95,r.max,r.worst,r.bias));}
  return p;
  }
  if let Some(reason)=self.capability.as_ref().and_then(|c|c.reason(Tool::Fit)){return p.child(div().text_color(rgb(0xfbbf24)).child(i18n::diagnostic(reason)));}
  if dynamic{return p.child(tr("N-color ICC fitting is not supported by Argyll colprof. Export measured data for a compatible profiling engine; no ICC is fabricated."));}
  p.child(div().text_lg().child(tr("Profile measured printing condition (Argyll colprof)")))
   .child(tr("Supply an actual CMYK .ti3 file from a measurement workflow. Builds a new ICC with explicit TAC and Argyll default black generation. Validate it before production. No automatic compensation or matching is assumed."))
   .child(button("colprof","Build ICC from measured TI3").on_click(cx.listener(|v,_,_,cx|{
    if v.busy||v.display_profile.as_ref().is_none_or(|p|!p.cmyk()){return}let Some(source)=rfd::FileDialog::new().add_filter("Measured Argyll TI3",&["ti3"]).pick_file()else{return};
    let Some(mut dest)=rfd::FileDialog::new().add_filter("ICC",&["icc"]).set_file_name("measured_profile.icc").save_file()else{return};if dest.extension().is_none(){dest.set_extension("icc");}
    let tac=v.tac;v.busy=true;v.status="Argyll colprof is building measured profile…".into();cx.notify();
    cx.spawn(async move|this,cx|{
     let result=cx.background_spawn(async move{lab::profile_from_ti3(&source,&dest,tac).map(|_|dest)}).await;
     let _=this.update(cx,|v,cx|{v.busy=false;match result{Ok(p)=>{v.status=format!("Saved {}",p.display());v.candidate=Some(p);v.preview=None;v.comparison=None;v.gradient=None;v.gradient_detail=None;if v.preview_open{v.refresh_gamut(cx);}},Err(e)=>v.status=format!("Error: {e}")}cx.notify();});
    }).detach();
   })))
 }
 #[cfg(windows)]
 fn update_poll(&self,cx:&mut Context<Self>){
  cx.spawn(async move|this,cx|{
   loop{
    cx.background_executor().timer(Duration::from_millis(150)).await;
    let state=this.update(cx,|v,cx|{let status=v.updater.status();if let UpdateStatus::Available(info)=&status{if v.notified_version.as_deref()!=Some(info.version.as_str()){v.notified_version=Some(info.version.clone());v.update_notice=Some(info.version.clone());}}else if !matches!(status,UpdateStatus::Checking){v.update_notice=None;}cx.notify();matches!(status,UpdateStatus::Checking|UpdateStatus::Downloading{..})});
    if !matches!(state,Ok(true)){break}
   }
  }).detach();
 }
 fn about_page(&self,cx:&mut Context<Self>)->Div{
  let mut updates=div().flex().flex_col().gap_2().p_4().rounded_md().bg(rgb(0x282828))
   .child(div().flex().flex_wrap().items_center().justify_between().gap_2().child(div().text_lg().child(tr("Software updates"))));
  #[cfg(windows)]{
   let state=self.updater.status();
   let checking=matches!(&state,UpdateStatus::Checking|UpdateStatus::Downloading{..});
   let staged=matches!(&state,UpdateStatus::Ready(..));
   updates=updates.child(div().flex().flex_wrap().gap_2()
    .child(button("check-studio",if checking{"Working…"}else if staged{"Verified update ready"}else{"Check for updates"}).bg(rgb(if checking||staged{0x383838}else{0xc65f24})).on_click(cx.listener(|v,_,_,cx|{if v.updater.start_check(false){v.update_poll(cx);cx.notify();}})))
    .child(button("github-releases-top","Releases").on_click(cx.listener(|_,_,_,cx|cx.open_url("https://github.com/emadgh/color-convert-rust/releases")))));
   match state{
    UpdateStatus::Idle=>updates=updates.child(tr("Check for a compatible Studio release.")),
    UpdateStatus::Checking=>updates=updates.child(tr("Checking GitHub Releases…")),
    UpdateStatus::UpToDate=>updates=updates.child(tr("No newer compatible Studio beta found.")),
    UpdateStatus::Available(info)=>updates=updates.child(div().text_color(rgb(0xfb923c)).child(format!("{}: {}",tr("New update available"),info.version))).child(button("download-studio","Download verified update").on_click(cx.listener(|v,_,_,cx|{if !v.busy&&v.updater.start_download(){v.update_notice=None;v.update_poll(cx);cx.notify();}}))),
    UpdateStatus::Downloading{info,downloaded,total}=>{
     updates=updates.child(format!("{} {} · {:.1} MiB",tr("Downloading"),info.version,downloaded as f64/1048576.));
     if let Some(n)=total.filter(|n|*n>0){updates=updates.child(bar((downloaded as f64/n as f64).clamp(0.,1.),0xe87935,10.));}
    }
    UpdateStatus::Ready(info,_)=>updates=updates.child(format!("{}: {}",tr("Verified update ready"),info.version)).child(button("install-studio","Restart and install").on_click(cx.listener(|v,_,_,cx|{if v.busy{return}match v.updater.apply_ready(){Ok(true)=>cx.quit(),Ok(false)=>{v.status="No staged update".into();cx.notify();},Err(e)=>{v.status=e;cx.notify();}}}))),
    UpdateStatus::Failed(e)=>{
     updates=updates.child(div().text_color(rgb(0xfca5a5)).child(i18n::diagnostic(&e)));
     if self.updater.can_retry_download(){updates=updates.child(button("retry-studio","Retry download").on_click(cx.listener(|v,_,_,cx|{if !v.busy&&v.updater.start_download(){v.update_poll(cx);cx.notify();}})));}
    },
   }
  }
  #[cfg(not(windows))]{updates=updates.child(tr("Automatic updates are available on Windows only."));}
  updates=updates.child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Studio beta releases only. Downloads require SHA-256 verification and explicit installation. Private releases require signed-in access.")));
  div().flex().flex_col().gap_3().child(updates)
   .child(div().p_4().rounded_md().bg(rgb(0x252525)).flex().flex_wrap().items_center().gap_4()
    .child(img(self.logo.clone()).size(px(80.)))
    .child(div().flex().flex_col().gap_1().child(div().text_2xl().child("ICC Profile Studio"))
     .child(div().text_sm().text_color(rgb(0xfb923c)).child(format!("{} · Windows x64 · GPUI",STUDIO_VERSION.trim())))
     .child("by emadghasemi.ir")))
   .child(div().p_3().rounded_md().bg(rgb(0x252525)).flex().flex_col().gap_2()
    .child(div().text_lg().child(tr("Developer"))).child("Emad Ghasemi")
    .child(div().flex().flex_wrap().gap_2()
     .child(button("developer-site","Website").on_click(cx.listener(|_,_,_,cx|cx.open_url("https://emadghasemi.ir"))))
     .child(button("github-project","Source code").on_click(cx.listener(|_,_,_,cx|cx.open_url("https://github.com/emadgh/color-convert-rust"))))
     .child(button("github-builds","Windows builds").on_click(cx.listener(|_,_,_,cx|cx.open_url("https://github.com/emadgh/color-convert-rust/actions"))))))
   .child(div().p_3().rounded_md().bg(rgb(0x252525)).flex().flex_col().gap_1()
    .child(div().text_lg().child(tr("Technology"))).child("Rust · GPUI · LittleCMS 2 · ArgyllCMS")
    .child(format!("Core package {}",env!("CARGO_PKG_VERSION")))
    .child("CMYK / Multichannel ICC · DeviceLink")
    .child(div().text_xs().child(cie_spectral::ATTRIBUTION)))
 }
 fn point_curves_page(&self,cx:&mut Context<Self>)->Div{
  div().flex().flex_col().gap_2().child(heading("Bounded multipoint curves","Drag three control points per ink. Endpoints locked; monotonic output enforced."))
   .child(self.curve_cards(true,cx))
   .child(tr("Piecewise-linear correction; independent B2A intents, source caps and explicit TAC. ICC v2 LUT16 or v4 mBA."))
   .child(seg("reset-points","Reset all points",false).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.curve_points.fill(IDENTITY_POINTS[0]);cx.notify();}})))
   .child(self.correction_options(cx))
   .child(button("save-points","Generate bounded curves ICC").on_click(cx.listener(|v,_,_,cx|v.save_edit(7,cx))))
 }
 fn black_page(&self,cx:&mut Context<Self>)->Div{
  if !self.display_profile.as_ref().is_some_and(|p|matches!(p.version_major(),2|4)&&p.pcs()=="Lab "&&color_convert_rust::multichannel::channel_count(p).is_ok()){return div().child(tr("Black generation requires compatible ICC v2 LUT16 or v4 mBA."))}
  let dynamic=self.display_profile.as_ref().is_some_and(|p|!p.cmyk());
  let user_caps=self.caps.clone();
  let bounds=self.display_profile.as_ref().and_then(|p|color_convert_rust::print_space::bounds(p).ok());
  let caps=bounds.as_ref().map(|b|b.channels.iter().zip(&user_caps).map(|(&x,&y)|(x*100.).min(y)).collect::<Vec<_>>()).unwrap_or_default();
  let effective_tac=self.correction_tac.min(self.tac);
  let s=self.black;
  let mut p=div().flex().flex_col().gap_3().child(heading("Black Generation / GCR / UCR","Explicit physical black channel · predictive constrained separation"))
   .child(tr("Select the physical black ink explicitly."))
   .child(tr("Names and swatches are identification aids only; confirm the physical ink order with your RIP. No black channel is inferred automatically."))
   .child(div().flex().flex_wrap().gap_2().children((0..self.gamma.len()).map(|k|{
     let cap=caps.get(k).copied().unwrap_or(0.);
     let active=cap>0.;
     let selected=self.black_channel==Some(k);
     let label=format!("{} · {} · {:.1}%{}",k+1,self.ink_name(k),cap,if selected{" ✓"}else{""});
     seg(("physical-black",k),label,selected).child(swatch(self.ink_color(k),12.)).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{if active{v.black_channel=Some(k);}else{v.status="Selected channel is closed or its limit cannot be established.".into();}cx.notify();}}))
   })))
   .child(format!("{}: {}",tr("Physical black channel"),self.black_channel.map(|k|format!("{} · {}",k+1,self.ink_name(k))).unwrap_or_else(||tr("Not selected").into())))
   .child(format!("{}: {:.1}%",tr("Effective TAC ceiling"),effective_tac))
   .child(tr("GCR favors broad chroma-weighted replacement; UCR is concentrated in dark near-neutrals. The requested black curve is a preference, not a guaranteed separation."))
   .child(div().flex().gap_2().child(button("gcr-mode",if !s.ucr{"GCR ✓"}else{"GCR"}).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.black.ucr=false;cx.notify();}})))
    .child(button("ucr-mode",if s.ucr{"UCR ✓"}else{"UCR"}).on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.black.ucr=true;cx.notify();}}))));
  let mut controls=div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828)).child(div().text_lg().child(tr("Black curve controls")));
  for (i,(name,value,step,lo,hi)) in [("Black start (darkness)",s.start,0.01,0.,0.95),("K target maximum",s.maximum,0.01,0.,1.),("Curve gamma",s.gamma,0.01,0.25,4.),("Preference strength",s.strength,0.01,0.,1.),("ΔE00 node tolerance",s.tolerance,0.1,0.1,5.)].into_iter().enumerate(){
   let label=if i==0||i==1||i==3{format!("{}: {:.0}%",tr(name),value*100.)}else{format!("{}: {value:.2}",tr(name))};
   let change=move|v:&mut Self,delta:f64|{let field=match i{0=>&mut v.black.start,1=>&mut v.black.maximum,2=>&mut v.black.gamma,3=>&mut v.black.strength,_=>&mut v.black.tolerance};*field=(*field+delta).clamp(lo,hi);};
   controls=controls.child(div().flex().flex_wrap().items_center().gap_2().child(div().w(px(165.)).child(label))
    .child(button(("black-minus",i),"−").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{change(v,-step);cx.notify();}})))
    .child(self.numeric_slider(("black-slider",i),value,lo,hi,if i==4{0.1}else{0.01},move|v,x|{*match i{0=>&mut v.black.start,1=>&mut v.black.maximum,2=>&mut v.black.gamma,3=>&mut v.black.strength,_=>&mut v.black.tolerance}=x;},cx)).child(button(("black-plus",i),"+").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{change(v,step);cx.notify();}}))));
  }
  let effective_max=self.black_channel.and_then(|k|caps.get(k)).map(|c|s.maximum.min(*c/100.)).unwrap_or(0.);
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
   .on_mouse_up(MouseButton::Left,cx.listener(|v,_,_,_|{v.curve_drag=None;}));
  p=p.child(div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828))
   .child(div().text_lg().child(tr("Requested black preference"))).child(graph)
   .child(div().flex().justify_between().text_xs().child(tr("0% darkness")).child(tr("100% darkness")))
   .child(div().flex().gap_2()
    .child(div().flex_1().min_w(px(0.)).child(format!("{}: {:.1}%",tr("Requested maximum"),s.maximum*100.)).child(bar(s.maximum,0xcbd5e1,12.)))
    .child(div().flex_1().min_w(px(0.)).child(format!("{}: {:.1}%",tr("Channel-capped maximum"),effective_max*100.)).child(bar(effective_max,0xe87935,12.))))
   .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Drag midpoint for gamma or right handle for maximum K. The solver remains constrained; requested curve is not guaranteed."))))
   .child(controls.child(button("reset-black","Reset").on_click(cx.listener(|v,_,_,cx|{if !v.busy{v.black=BlackGeneration::default();cx.notify();}}))));
  p.child(self.correction_options(cx))
   .child(tr("ICC v2 LUT16 or v4 mBA, 17/33 grid. Uses original A2B1 prediction and stricter source/Rebuilder limits. Legacy output also requires sampled color tolerance for all four intents. Not measured calibration or a continuous color-error guarantee."))
   .child(button("save-black","Generate black preference ICC").bg(rgb(0xc65f24)).on_click(cx.listener(|v,_,_,cx|v.save_edit(6,cx))))
 }
 fn preference_snapshot(&self)->Preferences{
  Preferences{schema:1,preview_open:self.preview_open,three_d:self.preview_options.three_d,wireframe:self.preview_options.wireframe,solid:self.preview_options.solid,profile_colors:self.preview_options.profile_colors,language:self.language,embedded_names:self.embedded_names,layout:self.preview_options.layout,floor_contour:self.preview_options.floor_contour,preview_width:self.preview_width,opacity_2d:self.preview_options.opacity_2d,gamut_grid:self.gamut_grid,compare_grid:self.compare_grid,chart_dpi:self.chart_dpi,gamut_reference_intent:intent_code(self.gamut_reference_intent),gamut_candidate_intent:intent_code(self.gamut_candidate_intent),gamut_separate_intents:self.gamut_separate_intents}
 }
 fn apply_preferences(&mut self,p:Preferences){
  self.preview_open=p.preview_open;self.preview_options.three_d=p.three_d;self.preview_options.wireframe=p.wireframe;self.preview_options.solid=p.solid;self.preview_options.profile_colors=p.profile_colors;self.language=p.language;self.embedded_names=p.embedded_names;self.preview_options.layout=p.layout;self.preview_options.floor_contour=p.floor_contour;self.preview_width=p.preview_width;self.preview_options.opacity_2d=p.opacity_2d;
  self.gamut_grid=p.gamut_grid;self.compare_grid=p.compare_grid;self.chart_dpi=p.chart_dpi;self.gamut_reference_intent=intent_from_code(p.gamut_reference_intent);self.gamut_candidate_intent=intent_from_code(p.gamut_candidate_intent);self.gamut_separate_intents=p.gamut_separate_intents;self.numeric_edit=None;
  self.preview=None;self.comparison=None;self.compare_error=None;
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
   .child(div().text_lg().child(tr("Language")))
   .child(div().flex().flex_wrap().gap_1().children(Language::ALL.into_iter().enumerate().map(|(i,l)|seg(("language",i),l.name(),self.language==l).on_click(cx.listener(move|v,_,_,cx|{v.language=l;i18n::set_active(l);if let Err(e)=preferences::location().and_then(|p|{let mut current=preferences::load(&p)?;current.language=l;preferences::save(&p,&current)}){v.status=format!("Language preference not saved: {e}");}cx.notify();})))))
   .child(tr("LittleCMS is linked statically. Official ArgyllCMS tools are installed by the portable backend when required."))
   .child(preferences::location().map(|p|p.display().to_string()).unwrap_or_else(|e|e))
   .child(tr("Save preferences explicitly to restore gamut preview mode, analysis grids, characterization chart DPI next launch. No profile paths, TAC, ink caps or running jobs are restored automatically."))
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
 fn inspector(&self,cx:&mut Context<Self>)->Div{
  let mut panel=div().flex().flex_col().gap_3().child(heading("Profile Inspector","ICC metadata decoded by LittleCMS 2."));
  if let Some(e)=&self.error{panel=panel.child(div().text_color(rgb(0xfca5a5)).child(i18n::diagnostic(e)));}
  if let Some(r)=&self.report{
   panel=panel.child(format!("ICC v{:.2}  |  {}  |  {}",r.version,r.color_space,r.pcs))
    .child(format!("Class: {}  |  {:.2} MiB",r.device_class,r.size as f64/1048576.))
    .child(format!("{}: {}",tr("Estimated total ink"),r.tac.map(|v|format!("{v:.1}%")).unwrap_or("Not available".into())))
    .child(format!("{} {}: {}",r.channel_names.len(),tr("Channels"),(0..r.channel_names.len()).map(|i|self.ink_name(i)).collect::<Vec<_>>().join(" · ")))
    .child(div().flex().flex_wrap().gap_2().children((0..r.channel_names.len()).map(|j|div().flex().items_center().gap_1().child(swatch(self.ink_color(j),16.)).child(div().flex().flex_col().child(div().text_xs().child(self.ink_name(j))).child(div().text_xs().text_color(rgb(0xa1a1aa)).child(self.ink_provenance(j)))))))
    ;
   if let Some(c)=&self.capability{panel=panel.child(div().text_lg().child(tr("Profile capabilities (preflight)")));for (tool,result) in &c.tools{panel=panel.child(div().flex().flex_col().gap_1().p_2().rounded_md().bg(rgb(0x282828)).child(format!("{} · {}",if result.is_ok(){"✓"}else{"⊘"},tr(tool.label()))).when(result.is_err(),|r|r.child(div().text_xs().text_color(rgb(0xfbbf24)).child(result.as_ref().err().cloned().unwrap_or_default()))));}panel=panel.child(div().text_xs().child(tr("Eligibility is structural/native preflight; final output validation is still required. Embedded names are not inferred; swatches may be single-ink predictions.")));}
   let mut white=div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828))
    .child(div().text_lg().child(if r.is_output{tr("Paper / Substrate Color")}else{tr("Media White Point")}))
    .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("ICC mediaWhitePointTag (wtpt), D50 PCS; not the header illuminant.")));
   if let Some(w)=&r.media_white{
    let mut details=div().flex().flex_col().gap_1()
     .child(format!("XYZ (Y=1 reference): {:.5} / {:.5} / {:.5}",w.xyz[0],w.xyz[1],w.xyz[2]))
     .child(format!("Lab D50: L* {:.2}  a* {:.2}  b* {:.2}",w.lab[0],w.lab[1],w.lab[2]));
    if let Some([red,green,blue])=w.srgb{
     let hex=(u32::from(red)<<16)|(u32::from(green)<<8)|u32::from(blue);
     details=details.child(format!("Approx. sRGB #{hex:06X}"));
     white=white.child(div().flex().items_center().gap_3().child(div().w(px(64.)).h(px(64.)).rounded_md().bg(rgb(hex))).child(details));
    }else{white=white.child(details);}
    white=white.child(div().text_xs().child(tr("Stored tag only. Swatch is an approximate sRGB rendering, not a calibrated or measured sample. A normalized D50 value does not prove the substrate is physically neutral.")));
   }else{white=white.child(tr("Not available: wtpt missing, unreadable or invalid. No substitute white is assumed."));}
   panel=panel.child(white);
   if let Some(profile)=&self.display_profile{
    let structures=color_convert_rust::advanced_lut::inspect(profile);
    if !structures.is_empty(){
     panel=panel.child(div().text_lg().child(tr("LUT structure")));
     for (tag,result) in structures{
      let mut card=div().flex().flex_col().gap_1().p_3().rounded_md().bg(rgb(0x282828));
      match result{
       Ok(l)=>{
        let axes=l.axes.iter().map(|v|v.to_string()).collect::<Vec<_>>().join(" × ");
        card=card.child(format!("{} · {} · {} → {} · {}-bit",l.tag,l.kind,l.inputs,l.outputs,l.precision_bits))
         .child(format!("CLUT: {}",axes))
         .child(format!("{}: {}  |  {}: {}  |  {}: {}",tr("Sampled curves"),l.sampled_curves,tr("Gamma curves"),l.gamma_curves,tr("Parametric curves"),l.parametric_curves))
         .child(format!("Matrix: {} · M curves: {}",if l.matrix{"✓"}else{"—"},if l.m_curves{"✓"}else{"—"}))
         .child(div().text_xs().child(tr("Structural decoding available; channel reorder still requires native validation.")));
        if tag.starts_with('B'){card=card.child(div().text_xs().child(tr(if l.strict_sampled_bounds{"Analytic device curve bounds supported"}else{"Device curve bounds cannot be certified"})));}
        for (j,result) in &l.device_curves{
         match result{
          Ok(c)=>{card=card.child(div().flex().items_center().gap_2().child(swatch(self.ink_color(*j),12.)).child(div().w(px(100.)).child(self.ink_name(*j))).child(format!("{} · {:.2}%–{:.2}%",c.kind,100.*c.minimum,100.*c.maximum)).child(div().text_xs().text_color(rgb(if c.monotonic{0xa7f3d0}else{0xfbbf24})).child(tr(if c.monotonic{"Nondecreasing"}else{"Non-monotonic"}))));},
          Err(e)=>{card=card.child(div().text_xs().text_color(rgb(0xfbbf24)).child(format!("CH {}: {}",j+1,e)));},
         }
        }
       },
       Err(e)=>card=card.child(format!("{tag}: {e}")),
      }
      panel=panel.child(card);
     }
     panel=panel.child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Complex LUT inspection and channel reorder do not imply support in version conversion or curve editing.")));
    }
   }
   panel=panel.child(div().text_xs().child(format!("{} ICC tags",r.tags.len())))
    .when(self.display_profile.is_some(),|p|p.child(button("save-rename","Save copy with new profile name").on_click(cx.listener(|v,_,_,cx|v.save_edit(2,cx)))));
  } else {panel=panel.child(tr("Open an ICC/ICM file or select an installed profile using System. Choose an editing or analysis tool from the sidebar."));}
  panel
 }
 fn validator(&self)->Div{
  let mut panel=div().flex().flex_col().gap_3().child(heading("Profile Validator","LittleCMS transform smoke checks; not proof of print accuracy."));
  if let Some(e)=&self.error{panel=panel.child(format!("Open failed: {e}"));}
  if let Some(r)=&self.report{
   panel=panel.child(tr("ICC opened by LittleCMS."));
   if r.intents.is_empty(){panel=panel.child(tr("Print-profile transform checks are not available for this color space."));}
   for check in &r.intents{
    let line=match &check.result{Ok(max)=>format!("Transforms created • sampled maxima {}",max.iter().map(|v|format!("{v:.2}%")).collect::<Vec<_>>().join(" / ")),Err(e)=>format!("Failed: {e}")};
    panel=panel.child(div().flex().flex_col().gap_1().p_3().rounded_md().bg(rgb(0x282828)).child(check.name).child(div().text_sm().child(line)));
   }
   panel=panel.child(tr("729 sRGB samples per intent. Sampled values are not exhaustive."));
   if let Some(bounds)=&r.multichannel_bounds{
    panel=panel.child(div().text_lg().child(tr("Strict LUT bounds")));
    match bounds{
     Ok(b)=>{for (j,v) in b.channels.iter().enumerate(){panel=panel.child(format!("{}: {:.4}%",self.ink_name(j),100.*v));}panel=panel.child(format!("TAC upper bound: {:.4}% ({})",100.*b.tac,if b.exact_node_tac{"affine output curves; exact node maximum"}else{"conservative global envelope; may overestimate"}));},
     Err(e)=>panel=panel.child(format!("Not certifiable with supported LUT parser: {e}")),
    }
    panel=panel.child(tr("Multichannel bounds are conservative; editing support depends on the LUT structure."));
   }
   if let Some(bounds)=&r.strict_bounds{
    panel=panel.child(div().text_lg().child(tr("Strict LUT bounds")));
    match bounds{
     Ok(b)=>{panel=panel.child(format!("Channel ceilings: {:.4?}%",b.channels.map(|v|v*100.))).child(format!("TAC upper bound: {:.4}% ({})",b.tac*100.,if b.exact_node_tac{"affine output curves; exact node maximum"}else{"conservative envelope; may overestimate"}));},
     Err(e)=>{panel=panel.child(format!("Not certifiable with supported LUT parser: {e}"));}
    }
   }
  }
  panel
 }
 fn rebuilder(&self,cx:&mut Context<Self>)->Div{
  let dynamic=self.display_profile.as_ref().is_some_and(|p|!p.cmyk());
  let mut p=div().flex().flex_col().gap_3().child(heading("Profile Rebuilder","Limit individual inks without changing the channel count."));
  if self.caps.is_empty(){return p.child(tr("Open a supported printer profile first."));}
  let caps=self.caps.clone();
  let tac=self.tac;let maximum=caps.len() as f64*100.;
  let active=caps.iter().filter(|&&v|v>0.).count();
  p=p.child(div().flex().gap_2()
   .child(div().flex_1().min_w(px(0.)).p_3().rounded_md().bg(rgb(0x282828)).child(div().text_xs().child(tr("Open channels"))).child(div().text_2xl().child(format!("{active} / {}",caps.len()))))
   .child(div().flex_1().min_w(px(0.)).p_3().rounded_md().bg(rgb(0x282828)).child(div().text_xs().child(tr("Requested TAC"))).child(div().text_2xl().text_color(rgb(0xfb923c)).child(format!("{tac:.0}%")))))
   .child(ink_budget_dynamic(&caps,tac))
   .child(div().p_3().rounded_md().bg(rgb(0x282828)).flex().flex_col().gap_2()
    .child(div().flex().items_center().justify_between().child(tr("Total ink coverage (TAC)")).child(format!("{tac:.0}%")))
    .child(div().flex().flex_wrap().items_center().gap_1()
     .child(button("rebuilder-tac-minus","−1").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{let t=&mut v.tac;*t=(*t-1.).max(0.);cx.notify();}})))
     .child(self.numeric_slider(("rebuilder-tac",0usize),tac,0.,maximum,1.,move|v,x|{v.tac=x},cx))
     .child(button("rebuilder-tac-plus","+1").on_click(cx.listener(move|v,_,_,cx|{if !v.busy{let t=&mut v.tac;*t=(*t+1.).min(maximum);cx.notify();}})))))
   .child(div().text_lg().child(tr("Channel limits")));
  for (i,&cap) in caps.iter().enumerate(){
   let color=self.ink_color(i);
   p=p.child(div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828))
    .child(div().flex().items_center().gap_2().child(swatch(color,22.)).child(div().flex_1().min_w(px(0.)).child(format!("{} · {}",i+1,self.ink_name(i)))).child(div().text_lg().child(format!("{cap:.0}%"))).child(div().text_xs().text_color(rgb(if cap==0.{0xa1a1aa}else{0xfb923c})).child(tr(if cap==0.{"Closed"}else{"Open"}))))
    .child(bar(cap/100.,color,16.))
    .child(div().flex().flex_wrap().gap_1().items_center()
     .child(seg(("rebuild-minus",i),"−1",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{let c=&mut v.caps[i];*c=(*c-1.).max(0.);cx.notify();}})))
     .child(self.numeric_slider(("rebuild-cap",i),cap,0.,100.,1.,move|v,x|{v.caps[i]=x},cx))
     .child(seg(("rebuild-plus",i),"+1",false).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{let c=&mut v.caps[i];*c=(*c+1.).min(100.);cx.notify();}})))
     .children([0usize,10,25,50,100].into_iter().map(|n|seg(("rebuild-preset",i*101+n),format!("{n}%"),cap==n as f64).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.caps[i]=n as f64;cx.notify();}}))))));
  }
  p=p.child(div().text_xs().text_color(rgb(0xa1a1aa)).child(tr("Channel bars use independent 0–100% scales. TAC constrains combined ink, not each individual maximum.")));
  if !dynamic&&self.display_profile.as_ref().is_some_and(|p|p.cmyk()){
   p=p.child(div().flex().flex_wrap().items_center().gap_1().child(tr("K presets:")).children([0usize,5,10,25,50,100].into_iter().map(|n|seg(("k-preset",n),format!("{n}%"),self.caps[3]==n as f64).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.caps[3]=n as f64;cx.notify();}})))));
  }
  p.child(div().flex().flex_col().gap_2().p_3().rounded_md().bg(rgb(0x282828))
    .child(div().text_lg().child(tr("Output settings")))
    .child(div().flex().flex_wrap().gap_2().items_center().child(format!("Grid: {}³",self.grid)).children([9usize,17,25,33].into_iter().map(|g|seg(("rebuild-grid",g),g.to_string(),self.grid==g).on_click(cx.listener(move|v,_,_,cx|{if !v.busy{v.grid=g;cx.notify();}})))))
    .child(format!("Engine: {}",if !dynamic&&self.mode==Mode::Argyll&&self.caps[..3].iter().all(|&v|v==100.){"ArgyllCMS revfix"}else{"Custom constrained LUT"}))
    .when(dynamic,|p|p.child(tr("Custom native A2B1 inversion. No assumed black channel. All three reverse intents are regenerated from relative-colorimetric prediction; this can change perceptual and saturation behavior. Higher grids and channel counts may be slow.")))
    .child(div().text_xs().child(tr("Confirm the physical ink order and safe TAC. Validate the printed result.")))
    .child(button("generate",if self.busy{"Working…"}else{"Generate and verify ICC"}).bg(rgb(0xc65f24)).on_click(cx.listener(|v,_,_,cx|v.generate(cx)))))

 }
}
impl Render for Studio {
 fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement{
  i18n::set_active(self.language);
  #[cfg(windows)] if !self.update_started{self.update_started=true;if self.updater.start_check(false){self.update_poll(cx);}}
  let viewport=window.viewport_size();let vw=f32::from(viewport.width);let vh=f32::from(viewport.height);
  let docked=ui_geometry::docked_preview(self.preview_open,vw);
  let width=self.preview_width.clamp(280.,(vw-760.).max(280.));
  let rtl=self.language.rtl();
  let display=(width-24.).min((vh-240.).max(180.));
  if self.preview_fullscreen{return div().flex().size_full().bg(rgb(0x101010)).text_color(rgb(0xe8e8e8)).child(self.preview_panel(vw,(vw-28.).min((vh-220.).max(180.)),true,cx));}
  let groups:[(&str,&[(Page,&str,&str)]);5]=[
   ("Inspection",&[(Page::Inspector,"Profile Inspector","◎"),(Page::Validator,"Profile Validator","✓")]),
   ("Editing",&[(Page::Rebuilder,"Profile Rebuilder","⟳"),(Page::Curves,"Tone Curves","∿"),(Page::Reorder,"Channel Reorder","⇅"),(Page::Correction,"Color Correction","◐"),(Page::Smoothing,"LUT Smoothing","≈"),(Page::Version,"ICC Version Conversion","⇄")]),
   ("Separation",&[(Page::Black,"Black Generation","◕"),(Page::Recipes,"Ink Optimizer","◈"),(Page::DeviceLink,"DeviceLink Generator","↔")]),
   ("Analysis",&[(Page::Comparison,"Profile Comparison","≍"),(Page::Gamut,"Gamut Viewer","◇"),(Page::Gradient,"Gradient Analyzer","▥"),(Page::Lab,"Profiling Lab","⊞")]),
   ("Application",&[(Page::Settings,"Settings","⚙"),(Page::About,"About & Updates","ⓘ")])];
  let mut brand=div().flex().items_center().gap_1()
   .child(div().id("brand-about").flex().flex_1().min_w(px(0.)).items_center().gap_2().rounded_md().cursor_pointer().hover(|s|s.bg(rgb(0x363636)))
    .child(img(self.logo.clone()).size(px(30.))).child(div().flex().flex_col().min_w(px(0.)).child(div().text_sm().child("ICC Profile Studio")).child(div().text_xs().text_color(rgb(0xa1a1aa)).child("by emadghasemi.ir")))
    .on_click(cx.listener(|v,_,_,cx|{v.page=Page::About;v.picker=None;v.preview_fullscreen=false;cx.notify();})));
  #[cfg(windows)] if matches!(self.updater.status(),UpdateStatus::Available(..)|UpdateStatus::Ready(..)){
   brand=brand.child(seg("brand-update","↑",true).bg(rgb(0xc65f24)).on_click(cx.listener(|v,_,_,cx|{v.page=Page::About;v.picker=None;v.update_notice=None;cx.notify();})));
  }
  let mut sidebar=div().id("sidebar").flex().flex_col().w(px(if vw<900.{200.}else{238.})).flex_shrink_0().h_full().overflow_y_scroll().p_3().gap_1().bg(rgb(0x262626)).when(rtl,|p|p.text_right())
   .child(brand)
   .child(div().text_xs().mb_2().text_color(rgb(0xa1a1aa)).child(STUDIO_VERSION.trim().to_owned()))
   .child(div().flex().flex_col().gap_1()
    .child(button("open-profile","Open ICC").bg(rgb(0xc65f24)).hover(|s|s.bg(rgb(0xe87935))).on_click(cx.listener(|v,_,_,cx|{v.open(cx);cx.notify();})))
    .child(self.system_dropdown(Picker::Reference,cx)))
   ;
  for (i,(group,items)) in groups.into_iter().enumerate(){
   let collapsed=self.nav_collapsed[i];
   let group_row=div().id(("nav-group",i)).mt_2().px_2().py_1().flex().items_center().justify_between().cursor_pointer().text_xs().text_color(rgb(0xa1a1aa));
   let arrow=if collapsed{if rtl{"◂"}else{"▸"}}else{"▾"};
   let group_row=if rtl{group_row.child(arrow).child(tr(group))}else{group_row.child(tr(group)).child(arrow)};
   sidebar=sidebar.child(group_row.on_click(cx.listener(move|v,_,_,cx|{v.nav_collapsed[i]=!v.nav_collapsed[i];cx.notify();})));
   if !collapsed{
    for &(page,label,icon) in items{
     let active=self.page==page;
     let mark=div().w(px(18.)).text_color(rgb(if active{0xfb923c}else{0xa1a1aa})).child(icon);
     let unavailable=self.page_reason(page).is_some();
     let label=div().flex_1().text_color(rgb(if unavailable{0x858585}else{0xe8e8e8})).child(tr(label)).when(unavailable,|l|l.child(" · ⊘"));
     let row=div().id(("page",page as usize)).px_2().py_1().rounded_md().flex().items_center().gap_2().text_sm().cursor_pointer()
      .bg(rgb(if active{0x543621}else{0x262626})).hover(|s|s.bg(rgb(0x363636)));
     let row=if self.language.rtl(){row.child(label.text_right()).child(mark)}else{row.child(mark).child(label)};
     sidebar=sidebar.child(row.on_click(cx.listener(move|v,_,_,cx|{v.page=page;v.numeric_edit=None;v.picker=None;if page==Page::Lab{v.ensure_lab_target(cx);}if page==Page::Gamut&&v.preview.is_none(){v.refresh_gamut(cx);}cx.notify();})));
    }
   }
  }
  let content=if let Some(reason)=self.page_reason(self.page){div().flex().flex_col().gap_3().child(div().text_lg().child(tr("Tool unavailable for this ICC"))).child(div().text_color(rgb(0xfbbf24)).child(i18n::diagnostic(reason))).child(tr("Open a compatible profile or inspect its LUT structure."))}else{match self.page{
   Page::Inspector=>self.inspector(cx),
   Page::Validator=>self.validator(),
   Page::Rebuilder=>self.rebuilder(cx),
   Page::Comparison=>self.comparison_page(cx),
   Page::Curves=>self.curves_page(cx),
   Page::Reorder=>self.reorder_page(cx),
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
  }};
  let mut editor=div().flex().flex_col().flex_1().min_w(px(0.)).h_full().bg(rgb(0x242424));
  #[cfg(windows)] if let Some(version)=&self.update_notice{editor=editor.child(div().flex().flex_wrap().items_center().gap_2().px_4().py_2().bg(rgb(0x543621)).child(div().flex_1().min_w(px(0.)).child(format!("{}: {version}",tr("New update available")))).child(button("notice-about","View update").on_click(cx.listener(|v,_,_,cx|{v.page=Page::About;v.update_notice=None;v.picker=None;cx.notify();}))).child(button("notice-dismiss","Later").on_click(cx.listener(|v,_,_,cx|{v.update_notice=None;cx.notify();}))));}
  let editor=editor
    .child(div().flex().flex_wrap().gap_2().items_center().justify_between().px_5().py_3().bg(rgb(0x282828)).child(div().flex_1().min_w(px(0.)).child(self.source.as_ref().and_then(|p|p.file_name()).map(|p|p.to_string_lossy().into_owned()).unwrap_or_else(||tr("No profile selected")))).when(self.display_profile.is_some(),|r|r.child(div().flex().gap_1().items_center()
     .child(seg("ink-name-standard","Standard names",!self.embedded_names).on_click(cx.listener(|v,_,_,cx|{v.embedded_names=false;v.persist_name_mode();cx.notify();})))
     .child(seg("ink-name-profile","Profile names",self.embedded_names).on_click(cx.listener(|v,_,_,cx|{v.embedded_names=true;v.persist_name_mode();cx.notify();})))
     .when(self.embedded_names&&self.display_profile.as_ref().is_some_and(|p|p.colorants().iter().all(Option::is_none)),|r|r.child(div().text_xs().text_color(rgb(0xfbbf24)).child(tr("No embedded ink names")))))).child(button("toggle-preview",if vw<1100.{"Open expanded gamut"}else if docked{"Hide preview"}else{"Show preview"}).on_click(cx.listener(move|this,_,_,cx|{if vw<1100.{this.preview_fullscreen=true;}else{this.preview_open=!this.preview_open;}if (this.preview_open||this.preview_fullscreen)&&this.preview.is_none(){this.refresh_gamut(cx);}cx.notify();}))))
    .child(div().id("content-scroll").flex_1().min_h(px(0.)).overflow_y_scroll().p_5().child(div().w_full().max_w(px(1000.)).mx_auto().when(rtl,|p|p.text_right()).child(content)))
    .child(self.analysis_progress(0,cx))
    .child(div().flex().items_center().gap_2().px_5().py_3().bg(rgb(0x262626)).text_sm().when(rtl,|p|p.flex_row_reverse()).child(div().size(px(8.)).rounded_md().bg(rgb(if self.busy{0xe87935}else{0x858585}))).child(div().flex_1().min_w(px(0.)).when(rtl,|r|r.text_right()).child(i18n::status(&self.status))).when(self.rebuild_control.is_some(),|r|r.child(button("cancel-rebuild",if self.rebuild_control.as_ref().is_some_and(|c|c.cancelled()){tr("Cancelling…")}else{tr("Cancel rebuild")}).on_click(cx.listener(|v,_,_,cx|{if let Some(c)=&v.rebuild_control{if c.cancel(){v.status="Cancelling…".into();cx.notify();}}})))));
  let mut root=div().flex().size_full().bg(rgb(0x242424)).text_color(rgb(0xe8e8e8));
  let resizer=div().id("gamut-resizer").w(px(6.)).h_full().flex_shrink_0().bg(rgb(0x383838)).hover(|s|s.bg(rgb(0xfb923c))).cursor_pointer()
    .on_mouse_down(MouseButton::Left,cx.listener(|v,e:&MouseDownEvent,_,cx|{v.resize_last=Some(e.position.x);cx.stop_propagation();}))
    .on_mouse_up(MouseButton::Left,cx.listener(|v,_,_,cx|{v.resize_last=None;cx.notify();}));
  if rtl{if docked{root=root.child(self.preview_panel(width,display,false,cx)).child(resizer);}root=root.child(editor).child(sidebar);}else{root=root.child(sidebar).child(editor);if docked{root=root.child(resizer).child(self.preview_panel(width,display,false,cx));}}
  root.on_mouse_move(cx.listener(move|v,e:&MouseMoveEvent,_,cx|{
    if !e.dragging(){v.resize_last=None;v.slider_drag=None;return}
    if let Some(last)=v.resize_last{v.preview_width=(v.preview_width+if rtl{f32::from(e.position.x-last)}else{-f32::from(e.position.x-last)}).clamp(280.,(vw-760.).max(280.));v.resize_last=Some(e.position.x);cx.notify();}
   }))
   .on_mouse_up(MouseButton::Left,cx.listener(|v,_,_,_|{v.resize_last=None;v.curve_drag=None;}))
 }
}
fn main(){
 #[cfg(windows)] unsafe{let id:Vec<u16>="ir.emadghasemi.ICCProfileStudio".encode_utf16().chain(Some(0)).collect();let _=windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(id.as_ptr());}
 Application::new().run(|cx:&mut App|{
  let bounds=Bounds::centered(None,size(px(1440.),px(880.)),cx);
  cx.open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(bounds)),titlebar:Some(TitlebarOptions{title:Some("ICC Profile Studio".into()),appears_transparent:false,..Default::default()}),..Default::default()},|window,cx|{window.set_window_title("ICC Profile Studio");cx.new(|_|Studio::startup())}).expect("Open Studio window");
  cx.activate(true);
 });
}
