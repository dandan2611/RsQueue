#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use axum::http::StatusCode;
use axum::Router;
use axum::routing::{
    get,
    put,
    delete
};
use log::{info, LevelFilter};
use once_cell::sync::Lazy;
use serde::ser::SerializeStruct;
use serde::Serialize;
use simple_logger::SimpleLogger;

struct QueueContext {
    queues: HashMap<String, VecDeque<Vec<String>>>,
    assigned_queue: HashMap<String, String>
}

impl QueueContext {
    fn new() -> Self {
        Self {
            queues: HashMap::new(),
            assigned_queue: HashMap::new()
        }
    }

    fn get_queue(&mut self, name: &str) -> &mut VecDeque<Vec<String>> {
        self.queues.entry(name.to_string()).or_insert(VecDeque::new())
    }

    fn push(&mut self, name: &str, value: Vec<String>) {
        value.iter()
            .for_each(|id| {
                self.assigned_queue.insert(id.to_string(), name.to_string());
            });
        self.get_queue(name).push_back(value);
    }

    fn pop(&mut self, name: &str) -> Option<Vec<String>> {
        self.get_queue(name).pop_front()
    }

    fn remove_player(&mut self, id: &str) {
        if let Some(queue_name) = self.assigned_queue.get(id) {
            if let Some(queue) = self.queues.get_mut(queue_name) {
                let position = queue.iter().position(|group| {
                    group.contains(&id.to_string())
                }).unwrap();
                let group = queue.get(position).unwrap();
                group.iter().for_each(|id| {
                    self.assigned_queue.remove(id);
                });
                queue.remove(position);
            }
        }
    }
}

static CONTEXT: Lazy<Mutex<QueueContext>> = Lazy::new(|| {
    Mutex::new(QueueContext::new())
});

async fn queue_add(
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::extract::Json(data): axum::extract::Json<serde_json::Value>
) -> Result<StatusCode, StatusCode> {
    let queue_name: String = id;
    let groups_arr: &Vec<serde_json::Value>;
    if let Some(group) = data.get("groups") {
        if let Some(arr) = group.as_array() {
            groups_arr = arr;
            if arr.len() == 0 {
                return Err(StatusCode::BAD_REQUEST);
            }
        } else {
            return Err(StatusCode::BAD_REQUEST);
        }
    } else {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut context_ctx = CONTEXT.lock().unwrap();
    for entity in groups_arr {
        let players_arr: &Vec<serde_json::Value> = entity.as_array().unwrap();
        let mut group = Vec::new();
        for id in players_arr {
            let id_reg = id.as_str();
            if id_reg.is_none() {
                return Err(StatusCode::BAD_REQUEST);
            }
            let id = id_reg.unwrap().to_string();
            if context_ctx.assigned_queue.contains_key(&id) {
                continue;
            }
            group.push(id);
        }

        if group.len() > 0 {
            context_ctx.push(&queue_name, group);
        }
    }
    Ok(StatusCode::OK)
}

async fn queue_remove(
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::extract::Json(data): axum::extract::Json<serde_json::Value>
) -> Result<StatusCode, StatusCode> {
    let queue_name: String = id;
    let players_arr: &Vec<serde_json::Value>;
    if let Some(players) = data.get("players") {
        if let Some(arr) = players.as_array() {
            players_arr = arr;
            if arr.len() == 0 {
                return Err(StatusCode::BAD_REQUEST);
            }
        } else {
            return Err(StatusCode::BAD_REQUEST);
        }
    } else {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut queue_ctx = CONTEXT.lock().unwrap();
    for id in players_arr.iter() {
        let id_reg = id.as_str();
        if id_reg.is_none() {
            return Err(StatusCode::BAD_REQUEST);
        }
        let id = id_reg.unwrap().to_string();
        queue_ctx.remove_player(&id);
    }
    Ok(StatusCode::OK)
}

async fn queue_pop(
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>
) -> Result<String, StatusCode> {
    let count = params.get("count").unwrap_or(&"1".to_string()).parse::<usize>().unwrap_or(1);
    let mut queue = CONTEXT.lock().unwrap();
    let mut result = Vec::new();
    for _ in 0..count {
        if let Some(group) = queue.pop(&id) {
            result.push(group);
        } else {
            break;
        }
    }
    Ok(serde_json::to_string(&result).unwrap())
}

struct DumpedQueueEntry {
    position: usize,
    group: Vec<String>
}

impl Serialize for DumpedQueueEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("DumpedQueueEntry", 2)?;
        state.serialize_field("position", &self.position)?;
        state.serialize_field("group", &self.group)?;
        state.end()
    }
}

async fn queue_dump(
    axum::extract::Path(id): axum::extract::Path<String>
) -> Result<String, StatusCode> {
    let queue = CONTEXT.lock().unwrap();
    let mut result = Vec::new();
    if let Some(queue) = queue.queues.get(&id) {
        for (i, group) in queue.iter().enumerate() {
            result.push(DumpedQueueEntry {
                position: i,
                group: group.clone()
            });
        }
    }
    Ok(serde_json::to_string(&result).unwrap())
}

struct PlayerQueueInfo {
    queue: String,
    position: usize,
    group: Vec<String>
}

impl Serialize for PlayerQueueInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("PlayerQueueInfo", 3)?;
        state.serialize_field("queue", &self.queue)?;
        state.serialize_field("position", &self.position)?;
        state.serialize_field("group", &self.group)?;
        state.end()
    }
}

async fn player_queue_info(
    axum::extract::Path(id): axum::extract::Path<String>
) -> Result<String, StatusCode> {
    let queue_ctx = CONTEXT.lock().unwrap();
    let mut player_group: Option<PlayerQueueInfo> = None;
    queue_ctx.assigned_queue.get(&id).map(|queue_name| {
        let queue = queue_ctx.queues.get(queue_name).unwrap();
        for (i, group) in queue.iter().enumerate() {
            for player in group {
                if *player == id {
                    player_group = Some(PlayerQueueInfo {
                        queue: queue_name.to_string(),
                        position: i,
                        group: group.to_vec()
                    });
                }
            }
        }
    });
    if player_group.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(serde_json::to_string(&player_group).unwrap())
}

#[tokio::main]
async fn main() {
    SimpleLogger::new()
        .with_level(LevelFilter::Debug)
        .init().unwrap();

    let app = Router::new()
        .route("/player/:id/queue", get(player_queue_info))
        .route("/queue/:id/add", put(queue_add))
        .route("/queue/:id/pop", get(queue_pop))
        .route("/queue/:id/dump", get(queue_dump))
        .route("/queue/:id/remove", delete(queue_remove));

    info!("RsQueue server started!");

    axum::Server::bind(&"127.0.0.1:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}