PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    salt CHAR(64) NOT NULL,
    hashed_password CHAR(64) NOT NULL,
    recovery_key CHAR(64) NOT NULL,
    created_time INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS user_sessions (
    id INTEGER PRIMARY KEY,
    user INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    session_key CHAR(64) NOT NULL UNIQUE,
    active BOOLEAN DEFAULT TRUE NOT NULL,
    start_time INTEGER NOT NULL,
    end_time INTEGER
);

CREATE TABLE IF NOT EXISTS media (
    id INTEGER PRIMARY KEY,
    media_key CHAR(16) NOT NULL UNIQUE,
    user INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    relative_path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    hashed_value CHAR(64) NOT NULL,
    UNIQUE(user, hashed_value)
);

CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY,
    project_key CHAR(16) NOT NULL,
    user INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    title TEXT
);

CREATE TABLE IF NOT EXISTS scenes (
    id INTEGER PRIMARY KEY,
    scene_key CHAR(16) NOT NULL,
    project INTEGER REFERENCES projects(id) ON DELETE CASCADE NOT NULL,
    title TEXT,
    w INTEGER NOT NULL,
    h INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS layers (
    id INTEGER NOT NULL,
    scene INTEGER REFERENCES scenes(id) ON DELETE CASCADE NOT NULL,
    title TEXT,
    z INTEGER,
    visible INTEGER,
    locked INTEGER,
    UNIQUE(id, scene)
);

CREATE TABLE IF NOT EXISTS sprites (
    id INTEGER NOT NULL,
    scene INTEGER REFERENCES scenes(id) ON DELETE CASCADE NOT NULL,
    layer INTEGER NOT NULL,
    media_key CHAR(16) REFERENCES media(media_key) ON DELETE SET NULL,
    r REAL,
    g REAL,
    b REAL,
    a REAL,
    x REAL NOT NULL,
    y REAL NOT NULL,
    w REAL NOT NULL,
    h REAL NOT NULL,
    z INTEGER NOT NULL,
    UNIQUE(id, scene)
);
