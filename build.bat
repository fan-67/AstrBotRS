@echo off
chcp 65001 >nul 2>&1
setlocal enabledelayedexpansion

cd /d D:\fannn\temp\astrbot_rs
if %errorlevel% neq 0 (
    echo [错误] 找不到项目目录 D:\fannn\temp\astrbot_rs
    pause
    exit /b 1
)

echo ============================================
echo  AstrBotRS Docker 构建脚本
echo ============================================
echo.
echo  工作目录: %cd%
echo.

where docker >nul 2>&1
if %errorlevel% neq 0 (
    echo [错误] 未找到 docker 命令
    echo 请确认 Docker Desktop 正在运行
    pause
    exit /b 1
)

echo  第 1 步: 停止旧容器...
docker compose down >nul 2>&1

echo  第 2 步: 构建镜像（预计 8-15 分钟）...
echo  日志 → build.log（请勿关闭此窗口）

del build.done 2>nul
docker compose build > build.log 2>&1
set BUILD_RESULT=%errorlevel%

echo.
if %BUILD_RESULT% equ 0 (
    echo DONE>build.done
    echo  [成功] 构建完成！
    echo.
    docker images astrbot_rs-astrbot --format "  镜像大小: {{.Size}}"
) else (
    echo  [失败] 构建出错！错误码: %BUILD_RESULT%
    echo  请查看 build.log 内容
)

echo ============================================
pause
