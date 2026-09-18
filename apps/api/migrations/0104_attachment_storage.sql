-- 0104：附件对象存储开关（storage_backend = local | s3）。
-- local（缺省）＝ savedirectory 本地卷；s3 ＝ S3 兼容端点（MinIO/OSS/COS/R2/B2），
-- 连接参数走环境变量 S3_ENDPOINT/S3_BUCKET/S3_KEY/S3_SECRET[/S3_REGION]。
-- 迁移期无缝：读取端当前后端未命中自动回落另一后端；对象键与本地卷同目录结构
-- （sha 两级分片），存量数据 rsync 到 bucket 即完成搬迁，无需改库。
INSERT INTO site_settings (name, value, grp, descr) VALUES
    ('storage_backend', 'local', 'main', '附件存储后端：local 本地卷 / s3 对象存储')
ON CONFLICT (name) DO NOTHING;
