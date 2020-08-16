CREATE TABLE vaulth (
    id           varchar(64) NOT NULL PRIMARY KEY,

    inserted_at  timestamptz NOT NULL,
    updated_at   timestamptz NOT NULL,

    name         varchar(64),
    about        text,

    password     varchar(256),

    google_id    varchar(256),
    microsoft_id varchar(256),
    facebook_id  varchar(256),
    twitter_id   varchar(256),
    github_id    varchar(256),
    discord_id   varchar(256)
);
