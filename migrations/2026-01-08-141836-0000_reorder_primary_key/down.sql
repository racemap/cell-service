-- Revert to original primary key order
ALTER TABLE cells DROP PRIMARY KEY,
    ADD PRIMARY KEY (radio, mcc, net, area, cell);
