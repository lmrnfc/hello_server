use std::{thread::{JoinHandle, self}, sync::{mpsc::{self, Sender, Receiver}, Arc, Mutex}, fmt::Display};

struct Worker {
    id: usize,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();
            match message {
                Ok(job) => {
                    // println!("Wokrer {id} got a job; executing.");
                    job()
                },
                Err(_) => {
                    // println!("Wokrer {id} disconnected; shutting down.");
                    break
                }
            }
        });
        Worker {
            id, 
            thread: Some(thread),
        }
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<Sender<Job>>, 
}

pub enum PoolCreationError {
    ZeroSize,
    ExcessSize((usize, usize)),
}

impl Display for PoolCreationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            PoolCreationError::ZeroSize => {
                String::from("can't have zero size pool")
            },
            PoolCreationError::ExcessSize(size) => {
                format!("excrss pool size: {} max is {}", size.0, size.1)
            },            
        };        
        write!(f, "{}", str)
    }
}

impl ThreadPool {
    fn new(size: usize) -> ThreadPool {
        assert!(size > 0);
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(size);       
        for id in 0..size {
            workers.push(Worker::new(id, receiver.clone()))
        }
        ThreadPool{
            workers,
            sender: Some(sender),
        }
    }
    pub fn build(size: usize) -> Result<ThreadPool, PoolCreationError>{
        if size == 0 {
            return Err(PoolCreationError::ZeroSize)    
        };
        let max_size = thread::available_parallelism().unwrap().get();
        if size > max_size {
            return Err(PoolCreationError::ExcessSize((size, max_size)))    
        };         
        Ok(Self::new(size))
    }
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.as_ref().unwrap().send(job).unwrap();     
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());
        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap()
            }
        }
    }
}
