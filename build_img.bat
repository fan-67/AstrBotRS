@echo off
cd /d D:\fannn\temp\astrbot_rs
docker build -t astrbot_rs-astrbot:latest . > build_img.log 2>&1
echo DONE>build_img.done
