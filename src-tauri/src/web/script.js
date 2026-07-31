"use strict";

let passwordInput = document.getElementById("pw");
if (passwordInput != null) {
    passwordInput.addEventListener("keydown", function (ev) {
        if (ev.keyCode == 13) {
            doLogin();
        }
    });
}

async function doLogin() {
    let e = document.getElementById("pw");
    if (e == null) throw "pw input is null.";
    let pw = e.value;
    if (pw.length < 1) {
        alert(passwordcantbeempty);
        return;
    }
    if (pw.length > 30) {
        alert(passwordistoolong);
        return;
    }
    try {
        let r = await fetch("/dologin", {
            method: "POST",
            headers: { "pw": pw },
        });
        let text = await r.text();
        if (text == "bad") {
            alert(wrongpassword);
        }
        location.href = location.href;
    } catch (err) {
        alert(someerror);
    }
}

let TaskListElement = document.getElementById("tasklist");
if (TaskListElement != null) {
    let CurrentProcesses = [];

    function BuildProcessHTML(p) {
        let div = document.createElement("div");
        div.className = "process " + p.ClassName;
        div.setAttribute("ps", p.Name);
        let h2 = document.createElement("h2");
        h2.innerText = p.Name;
        div.appendChild(h2);
        let span = document.createElement("span");
        div.appendChild(span);
        if (p.Title != null) {
            span.innerText = p.Title;
        }
        div.addEventListener("click", function (ev) {
            let psname = this.getAttribute("ps");
            if (psname == null) return;
            let s = areyousuretoclose + psname + "?";
            if (this.className == "process system") {
                s += " (" + thisissystemprocess + ")";
            }
            if (window.confirm(s)) {
                fetch("/kill", {
                    method: "POST",
                    headers: { "ps": psname },
                }).then(function (r) {
                    return r.text();
                }).then(function (s) {
                    if (s == "fail") {
                        alert(failtoclose);
                    } else if (s == "ok") {
                        alert(pleaselookatyourpc);
                    }
                    location.href = location.href;
                }).catch(function () {
                    alert(someerror);
                });
            }
        });
        return div;
    }

    function GetTaskList() {
        fetch("/list", { method: "POST" }).then(function (r) {
            return r.text();
        }).then(function (s) {
            if (s.length < 50) throw "bad task list json!";
            let list = JSON.parse(s);
            if (TaskListElement == null) throw "tasklist div is null";
            for (let i = 0; i < list.length; i++) {
                let p = list[i];
                let getin = false;
                let exist = false;
                for (let j = 0; j < CurrentProcesses.length; j++) {
                    let p2 = CurrentProcesses[j];
                    if (p2.Name == p.Name) {
                        let div = TaskListElement.children[j];
                        let span = div.getElementsByTagName("span").item(0);
                        if (span != null) {
                            span.innerText = p.Title != null ? p.Title : "";
                        }
                        div.className = "process " + p.ClassName;
                        exist = true;
                        break;
                    }
                    if (p2.Name > p.Name) {
                        TaskListElement.insertBefore(BuildProcessHTML(p), TaskListElement.children[j]);
                        CurrentProcesses.push(p);
                        getin = true;
                        break;
                    }
                }
                if (!exist) {
                    if (!getin) {
                        TaskListElement.appendChild(BuildProcessHTML(p));
                        CurrentProcesses.push(p);
                    }
                    CurrentProcesses.sort(function (a, b) {
                        if (a.Name > b.Name) return 1;
                        if (a.Name < b.Name) return -1;
                        return 0;
                    });
                }
            }
            for (let j = CurrentProcesses.length - 1; j >= 0; j--) {
                let p2 = CurrentProcesses[j];
                let exist = false;
                for (let i = 0; i < list.length; i++) {
                    if (list[i].Name == p2.Name) {
                        exist = true;
                        break;
                    }
                }
                if (!exist) {
                    TaskListElement.children[j].remove();
                    CurrentProcesses.splice(j, 1);
                }
            }
        }).catch(function () {
            // 连接失败时静默，下一轮再试
        });
    }

    setInterval(GetTaskList, 2000);
    GetTaskList();
}

let mainDiv = document.getElementById("main");
if (mainDiv == null) throw "main div is null.";
let width = document.body.clientWidth;
if (width > 500) width = 500;
mainDiv.style.width = width.toString() + "px";
mainDiv.style.marginLeft = "auto";
mainDiv.style.marginRight = "auto";

function translate(classname, txt) {
    let c = document.getElementsByClassName(classname);
    if (c != null && c.length > 0) {
        for (let i = 0; i < c.length; i++) {
            c.item(i).innerText = txt;
        }
    }
}

let language = "WEBLANGUAGE";
let apptitle = "Run Task Manager On Your Phone";
let timegoesout = "Connect timeout!";
let someerror = "There is an error when connect!";
let passwordcantbeempty = "Password can't be empty!";
let passwordistoolong = "Password is too long!";
let wrongpassword = "Wrong password!";
let areyousuretoclose = "Are you sure to close ";
let thisissystemprocess = "This is a system process";
let pleaselookatyourpc = "Please check your pc.";
let failtoclose = "Fail to close process.";

switch (language) {
    case "CN":
        apptitle = "手机任务管理器";
        translate("chooseapp", "点击你想要杀掉的程序");
        translate("bywalkedby", "By 戈登走過去");
        translate("tologin", "登录");
        timegoesout = "连接超时！";
        someerror = "连接出现未知错误！";
        areyousuretoclose = "你确定要关闭";
        thisissystemprocess = "这是一个系统进程";
        pleaselookatyourpc = "请看你的电脑。";
        failtoclose = "无法关闭进程";
        translate("enteryourpassword", "请输入你的密码：");
        passwordcantbeempty = "密码不能为空！";
        wrongpassword = "密码错误！";
        passwordistoolong = "密码太长！";
        break;
    case "ZHTW":
        apptitle = "手機工作管理員";
        translate("chooseapp", "點擊你想要殺掉的程式");
        translate("bywalkedby", "By 戈登走過去");
        translate("tologin", "登錄");
        timegoesout = "連接超時！";
        someerror = "連接出現未知錯誤！";
        areyousuretoclose = "你確定要關閉";
        thisissystemprocess = "這是一個系統相關程式";
        pleaselookatyourpc = "請看你的電腦。";
        failtoclose = "無法關閉該程式";
        translate("enteryourpassword", "請輸入你的密碼：");
        passwordcantbeempty = "密碼不能為空！";
        wrongpassword = "密碼錯誤！";
        passwordistoolong = "密碼太長！";
        break;
    default:
        break;
}

translate("apptitle", apptitle);
document.title = apptitle;
