-- Add CDMA to the radio enum
ALTER TABLE cells MODIFY COLUMN radio ENUM('gsm','umts','lte','nr','cdma') NOT NULL;
