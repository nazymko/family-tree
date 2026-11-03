CREATE TRIGGER [tDD_ad] AFTER DELETE ON [DocumentDetails]
BEGIN
  DELETE FROM
    [Recs]
  WHERE
    [rec_table] = 3 AND [rec_id] = [old].[id];
END;

CREATE TRIGGER [tDD_ai] AFTER INSERT ON [DocumentDetails]
BEGIN
  INSERT INTO [Recs]
    (
    [rec_table], 
    [rec_id])
    VALUES (3, [new].[id]);
END;

CREATE TRIGGER [tD_ad] AFTER DELETE ON [Documents]
BEGIN
  DELETE FROM
    [Recs]
  WHERE
    [rec_table] = 4 AND [rec_id] = [old].[id];
END;

CREATE TRIGGER [tD_ai] AFTER INSERT ON [Documents]
BEGIN
  INSERT INTO [Recs]
    (
    [rec_table], 
    [rec_id])
    VALUES (4, [new].[id]);
END;

CREATE TRIGGER [tED_ad] AFTER DELETE ON [EventDetails]
BEGIN
  DELETE FROM
    [Recs]
  WHERE
    [rec_table] = 5
  AND [rec_id] = [old].[id];
END;

CREATE TRIGGER [tED_ai] AFTER INSERT ON [EventDetails]
BEGIN
  INSERT INTO [Recs] ([rec_table], [rec_id]) VALUES (5, [new].[id]);
END;

CREATE TRIGGER [tER_ad] AFTER DELETE ON [EventRoles]
BEGIN
  DELETE FROM
    [Recs]
  WHERE
    [rec_table] = 6 AND [rec_id] = [old].[id];
END;

CREATE TRIGGER [tER_ai] AFTER INSERT ON [EventRoles]
BEGIN
  INSERT INTO [Recs]
    (
    [rec_table], 
    [rec_id])
    VALUES (6, [new].[id]);
END;

CREATE TRIGGER [tET_ad] AFTER DELETE ON [EventTypes]
BEGIN
  DELETE FROM
    [Recs]
  WHERE
    [rec_table] = 8 AND [rec_id] = [old].[id];
END;

CREATE TRIGGER [tET_ai] AFTER INSERT ON [EventTypes]
BEGIN
  INSERT INTO [Recs]
    (
    [rec_table], 
    [rec_id])
    VALUES (8, [new].[id]);
END;

CREATE TRIGGER [tE_ad] AFTER DELETE ON [Events]
BEGIN
  DELETE FROM
    [Recs]
  WHERE
    [rec_table] = 7
  AND [rec_id] = [old].[id];
END;

CREATE TRIGGER [tE_ai] AFTER INSERT ON [Events]
BEGIN
  INSERT INTO [Recs]
    (
    [rec_table], 
    [rec_id])
    VALUES (7, [new].[id]);
END;

CREATE TRIGGER [tF_ad] AFTER DELETE ON [Fields]
BEGIN
  DELETE FROM
    [Recs]
  WHERE
    [rec_table] = 10 AND [rec_id] = [old].[id];
END;

CREATE TRIGGER [tF_ai] AFTER INSERT ON [Fields]
BEGIN
  INSERT INTO [Recs]
    (
    [rec_table], 
    [rec_id])
    VALUES (10, [new].[id]);
END;

CREATE TRIGGER [tFam_ad] AFTER DELETE ON [Families]
BEGIN
  DELETE FROM
    [Recs]
  WHERE
    [rec_table] = 9
  AND [rec_id] = [old].[id];
END;

CREATE TRIGGER [tFam_ai] AFTER INSERT ON [Families]
BEGIN
  INSERT INTO [Recs]
    (
    [rec_table], 
    [rec_id])
    VALUES (9, [new].[id]);
END;

CREATE TRIGGER [tP_ad] AFTER DELETE ON [Persons]
BEGIN
  DELETE FROM
    [Recs]
  WHERE
    [rec_table] = 13
  AND [rec_id] = [old].[id];
END;

CREATE TRIGGER [tP_ai] AFTER INSERT ON [Persons]
BEGIN
  INSERT INTO [Recs]
    (
    [rec_table], 
    [rec_id])
    VALUES (13, [new].[id]);
END;

CREATE TRIGGER [tPl_ad] AFTER DELETE ON [Places]
BEGIN
  DELETE FROM
    [Recs]
  WHERE
    [rec_table] = 14 AND [rec_id] = [old].[id];
END;

CREATE TRIGGER [tPl_ai] AFTER INSERT ON [Places]
BEGIN
  INSERT INTO [Recs]
    (
    [rec_table], 
    [rec_id])
    VALUES (14, [new].[id]);
END;

CREATE TRIGGER [tSD_ad] AFTER DELETE ON [SourceDetails]
BEGIN
DELETE FROM Recs WHERE rec_table=15 AND rec_id=old.id;
END;

CREATE TRIGGER [tSD_ai] AFTER INSERT ON [SourceDetails]
BEGIN
INSERT INTO Recs (rec_table,rec_id) VALUES (15,new.id);
END;

CREATE TRIGGER [tS_ad] AFTER DELETE ON [Sources]
BEGIN
  DELETE FROM
    [Recs]
  WHERE
    [rec_table] = 16
  AND [rec_id] = [old].[id];
END;

CREATE TRIGGER [tS_ai] AFTER INSERT ON [Sources]
BEGIN
  INSERT INTO [Recs]
    (
    [rec_table], 
    [rec_id])
    VALUES (16, [new].[id]);
END;

CREATE TRIGGER [tTD_ad] AFTER DELETE ON [TaskDetails]
BEGIN
DELETE FROM Recs WHERE rec_table=21 AND rec_id=old.id;
END;

CREATE TRIGGER [tTD_ai] AFTER INSERT ON [TaskDetails]
BEGIN
INSERT INTO Recs (rec_table,rec_id) VALUES (21,new.id);
END;

CREATE TRIGGER [tT_ad] AFTER DELETE ON [Tasks]
BEGIN
  DELETE FROM
    [Recs]
  WHERE
    [rec_table] = 18 AND [rec_id] = [old].[id];
END;

CREATE TRIGGER [tT_ai] AFTER INSERT ON [Tasks]
BEGIN
  INSERT INTO [Recs]
    (
    [rec_table], 
    [rec_id])
    VALUES (18, [new].[id]);
END;

CREATE TABLE [DocumentDetails](
  [id] INTEGER PRIMARY KEY, 
  [d_id] INTEGER NOT NULL REFERENCES [Documents]([id]) ON DELETE CASCADE ON UPDATE CASCADE, 
  [rec_table] INTEGER NOT NULL, 
  [rec_id] INTEGER NOT NULL, 
  [d_ord] INTEGER, 
  [rec_ord] INTEGER, 
  [ismain] INTEGER, 
  [x1] REAL, 
  [y1] REAL, 
  [x2] REAL, 
  [y2] REAL, 
  [preview] BLOB, 
  [previewdate] DATETIME, 
  FOREIGN KEY([rec_table], [rec_id]) REFERENCES [Recs]([rec_table], [rec_id]) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE [Documents](
  [id] INTEGER PRIMARY KEY, 
  [path] TEXT NOT NULL, 
  [preview] BLOB, 
  [previewdate] DATETIME, 
  [previewprop] REAL);

CREATE TABLE [EventDetails](
  [id] INTEGER PRIMARY KEY, 
  [p_id] INTEGER NOT NULL REFERENCES [Persons]([id]) ON DELETE CASCADE ON UPDATE CASCADE, 
  [e_id] INTEGER NOT NULL REFERENCES [Events]([id]) ON DELETE CASCADE ON UPDATE CASCADE, 
  [er_id] INTEGER NOT NULL REFERENCES [EventRoles]([id]) ON DELETE CASCADE ON UPDATE CASCADE, 
  [e_ord] INTEGER, 
  [p_ord] INTEGER);

CREATE TABLE [EventRoles](
  [id] INTEGER PRIMARY KEY, 
  [et_id] INTEGER REFERENCES [EventTypes]([id]) ON DELETE CASCADE ON UPDATE CASCADE, 
  [maxcount] INTEGER, 
  [ord] INTEGER, 
  [roletype] INTEGER, 
  [ismain] INTEGER);

CREATE TABLE [EventTypes](
  [id] INTEGER PRIMARY KEY, 
  [ord] INTEGER NOT NULL);

CREATE TABLE [Events](
  [id] INTEGER PRIMARY KEY, 
  [et_id] INTEGER NOT NULL REFERENCES [EventTypes]([id]) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE [Families](
  [id] INTEGER PRIMARY KEY, 
  [color] INTEGER);

CREATE TABLE [Fields](
  [id] INTEGER PRIMARY KEY, 
  [tablecode] INTEGER NOT NULL, 
  [datatype] INTEGER, 
  [area] TEXT, 
  [defval] INTEGER, 
  [noautofill] INTEGER, 
  [icon] BLOB, 
  [et_id] INTEGER REFERENCES [EventTypes]([id]) ON DELETE CASCADE ON UPDATE CASCADE, 
  [et_ord] INTEGER);

CREATE TABLE [Global](
  [id] INTEGER PRIMARY KEY, 
  [version] INTEGER, 
  [guid] TEXT, 
  [srcguid] TEXT, 
  [mainlang] TEXT, 
  [params] TEXT);

CREATE TABLE [Log](
  [id] INTEGER PRIMARY KEY, 
  [rec_table] INTEGER NOT NULL, 
  [rec_id] INTEGER NOT NULL, 
  [action] INTEGER, 
  [moment] DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, 
  [name] TEXT);

CREATE TABLE [Persons](
  [id] INTEGER PRIMARY KEY, 
  [sex] INTEGER);

CREATE TABLE [Places](
  [id] INTEGER PRIMARY KEY, 
  [parent_id] INTEGER REFERENCES [Places]([id]) ON DELETE CASCADE ON UPDATE CASCADE, 
  [group_id] INTEGER, 
  [maskfull] INTEGER, 
  [maskshort] INTEGER, 
  [global_id] BLOB);

CREATE TABLE [Recs](
  [id] INTEGER PRIMARY KEY, 
  [rec_table] INTEGER NOT NULL, 
  [rec_id] INTEGER NOT NULL, 
  [guid] BLOB NOT NULL DEFAULT (RANDOMBLOB (16)), 
  [di] DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, 
  [de] DATETIME, 
  [lconf] INTEGER, 
  [ltrust] INTEGER, 
  [fav] INTEGER);

CREATE TABLE [SourceDetails](
  [id] INTEGER PRIMARY KEY, 
  [s_id] INTEGER NOT NULL REFERENCES [Sources]([id]) ON DELETE CASCADE ON UPDATE CASCADE, 
  [rec_table] INTEGER NOT NULL, 
  [rec_id] INTEGER NOT NULL, 
  [s_ord] INTEGER, 
  [rec_ord] INTEGER, 
  FOREIGN KEY([rec_table], [rec_id]) REFERENCES [Recs]([rec_table], [rec_id]) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE [Sources](
  [id] INTEGER PRIMARY KEY, 
  [stype] INTEGER);

CREATE TABLE [TaskDetails](
  [id] INTEGER PRIMARY KEY, 
  [t_id] INTEGER NOT NULL REFERENCES [Tasks]([id]) ON DELETE CASCADE ON UPDATE CASCADE, 
  [rec_table] INTEGER NOT NULL, 
  [rec_id] INTEGER NOT NULL, 
  [t_ord] INTEGER, 
  [rec_ord] INTEGER, 
  FOREIGN KEY([rec_table], [rec_id]) REFERENCES [Recs]([rec_table], [rec_id]) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE [Tasks]([id] INTEGER PRIMARY KEY);

CREATE TABLE [ValuesDates](
  [id] INTEGER PRIMARY KEY, 
  [f_id] INTEGER NOT NULL REFERENCES [Fields]([id]) ON DELETE CASCADE ON UPDATE CASCADE, 
  [rec_table] INTEGER NOT NULL, 
  [rec_id] INTEGER NOT NULL, 
  [d] INTEGER, 
  [m] INTEGER, 
  [y] INTEGER, 
  [d2] INTEGER, 
  [m2] INTEGER, 
  [y2] INTEGER, 
  [calendar] INTEGER, 
  [calendar2] INTEGER, 
  [type] INTEGER, 
  [sort] INTEGER, 
  [sort2] INTEGER, 
  [sort3] INTEGER, 
  [sort4] INTEGER, 
  [lconf] INTEGER, 
  [ltrust] INTEGER, 
  FOREIGN KEY([rec_table], [rec_id]) REFERENCES [Recs]([rec_table], [rec_id]) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE [ValuesLinks](
  [id] INTEGER PRIMARY KEY, 
  [f_id] INTEGER NOT NULL REFERENCES [Fields]([id]) ON DELETE CASCADE ON UPDATE CASCADE, 
  [rec_table] INTEGER NOT NULL, 
  [rec_id] INTEGER NOT NULL, 
  [vlink_table] INTEGER NOT NULL, 
  [vlink_id] INTEGER NOT NULL, 
  [lconf] INTEGER, 
  [ltrust] INTEGER, 
  FOREIGN KEY([rec_table], [rec_id]) REFERENCES [Recs]([rec_table], [rec_id]) ON DELETE CASCADE ON UPDATE CASCADE, 
  FOREIGN KEY([vlink_table], [vlink_id]) REFERENCES [Recs]([rec_table], [rec_id]) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE [ValuesNum](
  [id] INTEGER PRIMARY KEY, 
  [f_id] INTEGER NOT NULL REFERENCES [Fields]([id]) ON DELETE CASCADE ON UPDATE CASCADE, 
  [rec_table] INTEGER NOT NULL, 
  [rec_id] INTEGER NOT NULL, 
  [vnum] REAL, 
  [vnum2] REAL, 
  [lconf] INTEGER, 
  [ltrust] INTEGER, 
  FOREIGN KEY([rec_table], [rec_id]) REFERENCES [Recs]([rec_table], [rec_id]) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE [ValuesStr](
  [id] INTEGER PRIMARY KEY, 
  [f_id] INTEGER NOT NULL REFERENCES [Fields]([id]) ON DELETE CASCADE ON UPDATE CASCADE, 
  [rec_table] INTEGER NOT NULL, 
  [rec_id] INTEGER NOT NULL, 
  [lang] TEXT, 
  [vstr] TEXT, 
  [lconf] INTEGER, 
  [ltrust] INTEGER, 
  FOREIGN KEY([rec_table], [rec_id]) REFERENCES [Recs]([rec_table], [rec_id]) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE INDEX [iDD_doc]
ON [DocumentDetails](
  [d_id], 
  [rec_ord]);

CREATE INDEX [iDD_rec]
ON [DocumentDetails](
  [rec_table], 
  [rec_id], 
  [d_ord]);

CREATE INDEX [iDD_recmain]
ON [DocumentDetails](
  [rec_table], 
  [rec_id], 
  [ismain]);

CREATE UNIQUE INDEX [iDD_search]
ON [DocumentDetails](
  [d_id], 
  [rec_table], 
  [rec_id]);

CREATE INDEX [iED_E]
ON [EventDetails](
  [e_id], 
  [er_id]);

CREATE INDEX [iED_E2]
ON [EventDetails](
  [e_id], 
  [p_ord]);

CREATE INDEX [iED_ER] ON [EventDetails]([er_id]);

CREATE INDEX [iED_P]
ON [EventDetails](
  [p_id], 
  [er_id]);

CREATE INDEX [iED_P2]
ON [EventDetails](
  [p_id], 
  [e_ord]);

CREATE UNIQUE INDEX [iED_PE]
ON [EventDetails](
  [p_id], 
  [e_id]);

CREATE INDEX [iER_ET]
ON [EventRoles](
  [et_id], 
  [ord]);

CREATE INDEX [iE_ET] ON [Events]([et_id]);

CREATE INDEX [iF_et_id] ON [Fields]([et_id]);

CREATE INDEX [iF_tablecode] ON [Fields]([tablecode]);

CREATE INDEX [iLog]
ON [Log](
  [rec_table], 
  [rec_id]);

CREATE INDEX [iPl_group] ON [Places]([group_id]);

CREATE INDEX [iPl_parent] ON [Places]([parent_id]);

CREATE UNIQUE INDEX [iRecs]
ON [Recs](
  [rec_table], 
  [rec_id]);

CREATE INDEX [iSD_R]
ON [SourceDetails](
  [rec_table], 
  [rec_id], 
  [s_ord]);

CREATE INDEX [iSD_S]
ON [SourceDetails](
  [s_id], 
  [rec_ord]);

CREATE INDEX [iTD_R]
ON [TaskDetails](
  [rec_table], 
  [rec_id], 
  [t_ord]);

CREATE INDEX [iTD_S]
ON [TaskDetails](
  [t_id], 
  [rec_ord]);

CREATE INDEX [iVD_F]
ON [ValuesDates](
  [f_id], 
  [rec_id]);

CREATE INDEX [iVD_R]
ON [ValuesDates](
  [rec_table], 
  [rec_id]);

CREATE INDEX [iVL_F]
ON [ValuesLinks](
  [f_id], 
  [rec_id]);

CREATE INDEX [iVL_R]
ON [ValuesLinks](
  [rec_table], 
  [rec_id]);

CREATE INDEX [iVL_R2]
ON [ValuesLinks](
  [vlink_table], 
  [vlink_id]);

CREATE INDEX [iVN_F]
ON [ValuesNum](
  [f_id], 
  [rec_id]);

CREATE INDEX [iVN_R]
ON [ValuesNum](
  [rec_table], 
  [rec_id]);

CREATE INDEX [iVS_F]
ON [ValuesStr](
  [f_id], 
  [rec_id], 
  [lang]);

CREATE INDEX [iVS_R]
ON [ValuesStr](
  [rec_table], 
  [rec_id]);
