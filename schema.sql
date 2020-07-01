CREATE TABLE users (
    id           VARCHAR(255) NOT NULL PRIMARY KEY,
    sign_up_at   DATETIME     NOT NULL,
    sign_in_at   DATETIME,

    password     VARCHAR(255),

    google_id    VARCHAR(255),
    microsoft_id VARCHAR(255),
    facebook_id  VARCHAR(255),
    twitter_id   VARCHAR(255),
    github_id    VARCHAR(255),
    discord_id   VARCHAR(255),
    steam_id     VARCHAR(255)
);
