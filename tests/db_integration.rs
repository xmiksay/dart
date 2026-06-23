//! Live-Postgres persistence round-trip. Ignored by default — run with a DB up:
//!   docker compose up -d postgres
//!   DATABASE_URL=postgres://dartscore:dartscore@localhost:5432/dartscore \
//!     cargo test --test db_integration -- --ignored

use chrono::Utc;
use dartscore::db::{
    self,
    repository::{self, NewDart, NewGame, NewPlayer, PlayerError},
};
use uuid::Uuid;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://dartscore:dartscore@localhost:5432/dartscore".to_string())
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL (DATABASE_URL)"]
async fn save_list_and_get_roundtrip() {
    let conn = db::connect(&database_url()).await.expect("connect");
    db::run_migrations(&conn).await.expect("migrate");

    let now = Utc::now();
    let new_game = NewGame {
        mode: "x01".to_string(),
        config: serde_json::json!({ "start": 501, "in_rule": "Straight", "out_rule": "Double" }),
        cut_throat: None,
        winner_index: Some(0),
        started_at: now,
        finished_at: now,
        players: vec![
            NewPlayer {
                name: "Ann".to_string(),
                position: 0,
                final_result: "0".to_string(),
            },
            NewPlayer {
                name: "Bob".to_string(),
                position: 1,
                final_result: "120".to_string(),
            },
        ],
        darts: vec![NewDart {
            player_position: 0,
            turn_no: 0,
            dart_no: 0,
            ring: "Double".to_string(),
            segment: 20,
            value: 40,
        }],
    };

    let id = repository::save_finished_game(&conn, new_game)
        .await
        .expect("save");

    let summaries = repository::list_games(&conn).await.expect("list");
    let summary = summaries
        .iter()
        .find(|g| g.id == id)
        .expect("saved game listed");
    assert_eq!(summary.mode, "x01");
    assert_eq!(summary.players, vec!["Ann".to_string(), "Bob".to_string()]);
    assert_eq!(summary.winner.as_deref(), Some("Ann"));

    let detail = repository::get_game(&conn, id)
        .await
        .expect("get")
        .expect("game exists");
    assert_eq!(detail.players.len(), 2);
    assert_eq!(detail.darts.len(), 1);
    assert_eq!(detail.darts[0].value, 40);
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL (DATABASE_URL)"]
async fn players_crud_roundtrip() {
    let conn = db::connect(&database_url()).await.expect("connect");
    db::run_migrations(&conn).await.expect("migrate");

    // Unique per run so repeated runs don't trip the name uniqueness constraint.
    let name = format!("Ann-{}", Uuid::new_v4());
    let created = repository::create_player(&conn, name.clone())
        .await
        .expect("create");
    assert_eq!(created.name, name);

    // Duplicate name is rejected as a typed Duplicate (→ 409 at the route).
    match repository::create_player(&conn, name.clone()).await {
        Err(PlayerError::Duplicate) => {}
        other => panic!("expected Duplicate, got {other:?}"),
    }

    let listed = repository::list_players(&conn).await.expect("list");
    assert!(listed.iter().any(|p| p.id == created.id));

    let new_name = format!("Anna-{}", Uuid::new_v4());
    let renamed = repository::rename_player(&conn, created.id, new_name.clone())
        .await
        .expect("rename")
        .expect("exists");
    assert_eq!(renamed.name, new_name);

    // Renaming a missing id yields Ok(None) → 404 at the route.
    let missing = repository::rename_player(&conn, Uuid::new_v4(), "x".into())
        .await
        .expect("rename missing");
    assert!(missing.is_none());

    assert!(repository::delete_player(&conn, created.id)
        .await
        .expect("delete"));
    // Idempotent: deleting again removes nothing.
    assert!(!repository::delete_player(&conn, created.id)
        .await
        .expect("delete again"));
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL (DATABASE_URL)"]
async fn finished_game_links_saved_player() {
    let conn = db::connect(&database_url()).await.expect("connect");
    db::run_migrations(&conn).await.expect("migrate");

    let ann_name = format!("Ann-{}", Uuid::new_v4());
    let ann = repository::create_player(&conn, ann_name.clone())
        .await
        .expect("create");

    let now = Utc::now();
    let new_game = NewGame {
        mode: "x01".to_string(),
        config: serde_json::json!({ "start": 501 }),
        cut_throat: None,
        winner_index: Some(0),
        started_at: now,
        finished_at: now,
        players: vec![
            NewPlayer {
                name: ann_name.clone(),
                position: 0,
                final_result: "0".to_string(),
            },
            NewPlayer {
                name: format!("Bob-{}", Uuid::new_v4()),
                position: 1,
                final_result: "120".to_string(),
            },
        ],
        darts: vec![],
    };

    let id = repository::save_finished_game(&conn, new_game)
        .await
        .expect("save");
    let detail = repository::get_game(&conn, id)
        .await
        .expect("get")
        .expect("exists");

    let ann_row = detail
        .players
        .iter()
        .find(|p| p.name == ann_name)
        .expect("ann persisted");
    assert_eq!(ann_row.player_id, Some(ann.id));

    // The ad-hoc Bob name has no saved player, so it stays unlinked.
    let bob_row = detail
        .players
        .iter()
        .find(|p| p.position == 1)
        .expect("bob persisted");
    assert!(bob_row.player_id.is_none());
}
