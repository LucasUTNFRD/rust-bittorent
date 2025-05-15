pub struct ActorHandle<M> {
    sender: mpsc::Sender<M>,
}

pub struct Actor<M> {
    // fields
    receiver: mpsc::Receiver<M>,
}

impl Actor<M> {
    pub fn new() -> Self {
        todo!()
    }

    pub async fn run(mut actor: Actor<M>) {}
}

impl ActorHandle<M> {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(32);
        let actor = Actor::new(peer_id, peers, receiver);
        tokio::task::spawn(Actor::run(actor));

        Self { sender }
    }

    pub fn send(&self, message: M) {
        self.sender.send(message).await;
    }
}
