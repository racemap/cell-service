-- Remove CDMA from the radio enum (will fail if CDMA data exists)
ALTER TABLE cells MODIFY COLUMN radio ENUM('gsm','umts','lte','nr') NOT NULL;
