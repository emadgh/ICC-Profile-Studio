//! Scoped cooperative cancellation for read-only synchronous analysis workers.
//! No file publication is cancellable through this token.
use std::{cell::RefCell,sync::{Arc,Mutex,atomic::{AtomicBool,AtomicUsize,Ordering}}};
use crate::R;
#[derive(Default)]struct State{cancel:AtomicBool,steps:AtomicUsize,phase:Mutex<(&'static str,usize,usize)>}
#[derive(Clone,Default)]pub struct Job(Arc<State>);
#[derive(Clone,Copy,Debug)]pub struct Progress{pub phase:&'static str,pub completed:usize,pub total:usize,pub checkpoints:usize,pub cancelled:bool}
thread_local!{static ACTIVE:RefCell<Option<Job>>=const{RefCell::new(None)};}
struct Restore(Option<Job>);
impl Drop for Restore{fn drop(&mut self){ACTIVE.with(|a|{a.replace(self.0.take());});}}
impl Job{
 pub fn cancel(&self){self.0.cancel.store(true,Ordering::Release);}
 pub fn cancelled(&self)->bool{self.0.cancel.load(Ordering::Acquire)}
 pub fn same(&self,other:&Self)->bool{Arc::ptr_eq(&self.0,&other.0)}
 pub fn progress(&self)->Progress{let (phase,completed,total)=*self.0.phase.lock().unwrap();Progress{phase,completed,total,checkpoints:self.0.steps.load(Ordering::Relaxed),cancelled:self.cancelled()}}
 pub fn run<T>(&self,f:impl FnOnce()->R<T>)->R<T>{
  let _restore=Restore(ACTIVE.with(|a|a.replace(Some(self.clone()))));
  check()?;let result=f();check()?;result
 }
}
pub fn check()->R<()>{ACTIVE.with(|a|{if let Some(j)=a.borrow().as_ref(){if j.cancelled(){return Err("Analysis cancelled".into())}}Ok(())})}
pub fn checkpoint()->R<()>{check()?;ACTIVE.with(|a|{if let Some(j)=a.borrow().as_ref(){j.0.steps.fetch_add(1,Ordering::Relaxed);}});Ok(())}
pub fn phase(name:&'static str,total:usize)->R<()>{check()?;ACTIVE.with(|a|{if let Some(j)=a.borrow().as_ref(){*j.0.phase.lock().unwrap()=(name,0,total);}});Ok(())}
pub fn advance()->R<()>{checkpoint()?;ACTIVE.with(|a|{if let Some(j)=a.borrow().as_ref(){let mut p=j.0.phase.lock().unwrap();p.1=(p.1+1).min(p.2);}});Ok(())}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn scoped_progress_and_cancel(){let j=Job::default();assert!(j.run(||{phase("test",3)?;advance()?;assert_eq!(j.progress().completed,1);j.cancel();advance()}).is_err());assert!(checkpoint().is_ok());assert!(j.cancelled());assert!(j.run(||Ok(7)).is_err());}
 #[test]fn nested_scope_restored(){let a=Job::default();let b=Job::default();a.run(||{b.run(||{checkpoint()?;Ok(())})?;checkpoint()?;Ok(())}).unwrap();assert_eq!(a.progress().checkpoints,1);assert_eq!(b.progress().checkpoints,1);}
}
