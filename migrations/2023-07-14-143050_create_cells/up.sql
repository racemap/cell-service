-- Your SQL goes here
CREATE TABLE cells (
  radio ENUM('gsm','umts','lte') NOT NULL,
  mcc SMALLINT UNSIGNED NOT NULL,
  net SMALLINT UNSIGNED NOT NULL,
  area SMALLINT UNSIGNED NOT NULL,
  cell INT UNSIGNED NOT NULL,
  unit SMALLINT UNSIGNED,
  lon FLOAT NOT NULL,
  lat FLOAT NOT NULL,
  cell_range INT UNSIGNED NOT NULL,
  samples INT UNSIGNED NOT NULL,
  changeable BOOLEAN NOT NULL,
  created DATETIME NOT NULL,
  updated DATETIME NOT NULL,
  average_signal SMALLINT,
  PRIMARY KEY (radio, mcc, net, area, cell)
);