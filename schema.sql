CREATE TABLE users (
    id           VARCHAR(64)  NOT NULL PRIMARY KEY,
    inserted_at  DATETIME     NOT NULL,
    updated_at   DATETIME     NOT NULL,
    login_at     DATETIME     NOT NULL,

    display_name VARCHAR(64),
    about        TEXT,

    password     VARCHAR(256),

    google_id    VARCHAR(256),
    microsoft_id VARCHAR(256),
    facebook_id  VARCHAR(256),
    twitter_id   VARCHAR(256),
    github_id    VARCHAR(256),
    discord_id   VARCHAR(256)
);
