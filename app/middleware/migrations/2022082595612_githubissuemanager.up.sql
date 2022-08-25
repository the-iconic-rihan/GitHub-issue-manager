CREATE TABLE issues
(
	id              	TEXT PRIMARY KEY NOT NULL,
	title           	TEXT             NOT NULL,
	created_at      	TIMESTAMPTZ      NOT NULL,
	url             	TEXT             NOT NULL,
	labels          	TEXT[]           NOT NULL,
	repository_name 	TEXT             NOT NULL,
	fork_count      	BIGINT           NOT NULL,
	star_count      	BIGINT           NOT NULL,
	primary_language	TEXT             NOT NULL,
	updated_at          TIMESTAMPTZ      NOT NULL default current_timestamp
);
