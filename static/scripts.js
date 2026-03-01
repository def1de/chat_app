let input = document.getElementById("chat-input");
let chatBox = document.getElementById("chat");
let socket;
let username_field = document.getElementById("username");
let user_count_field = document.getElementById("user-count");

function getChatIdFromPath() {
  const m = window.location.pathname.match(/^\/chat\/(\d+)\/?$/);
  return m ? parseInt(m[1], 10) : null;
}

window.onload = function () {
  let chatId = getChatIdFromPath();
  if (!chatId) {
    return;
  }
  let socketUrl = `ws://localhost:1578/chatsocket/${chatId}`;
  socket = new WebSocket(socketUrl);
  socket.onopen = function () {
    updateUserCount();
    scrollToBottom();
  };

  socket.onclose = function () {
    alert("Connection closed. Please refresh the page to reconnect.");
  };

  socket.onmessage = (event) => {
    const parts = event.data.split(": ", 2);
    chatBox.innerHTML +=
      `<div class="message left">
                    <p class="username">` +
      parts[0] +
      `</p>
                    <p class="message_content">` +
      parts[1] +
      `</p>
                </div>`;
    scrollToBottom();
  };
};

document.getElementById("inviteBtn").onclick = function () {
  let chatId = getChatIdFromPath();
  if (!chatId) {
    return;
  }
  fetch(`/create_invite/${chatId}`, { method: "POST" })
    .then((response) => response.json())
    .then((data) => {
      if (data.code) {
        const inviteLink = `${window.location.origin}/invite/${data.code}`;
        navigator.clipboard.writeText(inviteLink).then(
          () => {
            alert("Invite link copied to clipboard:\n" + inviteLink);
          },
          (err) => {
            alert("Failed to copy invite link: " + err);
          },
        );
      } else {
        alert("Failed to create invite link.");
      }
    })
    .catch((error) => {
      console.error("Error creating invite link:", error);
      alert("Error creating invite link.");
    });
};

input.addEventListener("keydown", function (event) {
  if (event.key === "Enter") {
    event.preventDefault();
    const message = input.value;
    if (message.trim() !== "") {
      socket.send(message);

      input.value = "";
    }

    chatBox.innerHTML +=
      `<div class="message right">
                    <p class="username">You</p>
                    <p class="message_content">` +
      message +
      `</p>
                </div>`;
    scrollToBottom();
  }
});

function scrollToBottom() {
  chatBox.scrollTo({
    top: chatBox.scrollHeight,
    behavior: "smooth",
  });
}

function updateUserCount() {
  fetch("/status")
    .then((response) => response.json())
    .then((data) => {
      user_count_field.innerText = "Current users: " + data.connected_clients;
    })
    .catch((error) => {
      console.error("Error fetching status:", error);
      user_count_field.innerText = "Current users: 0";
    });
}

setInterval(updateUserCount, 10000);

(() => {
  const root = document.getElementById("chatCarousel");
  if (!root) return;

  const items = Array.from(root.querySelectorAll(".chat-option"));

  // Use current chat id from URL to pick the active item
  let active = 0;
  const currentId = getChatIdFromPath();
  if (currentId !== null) {
    const idx = items.findIndex((el) => {
      const a = el.closest("a");
      if (!a) return false;
      const m = a.getAttribute("href")?.match(/^\/chat\/(\d+)\/?$/);
      return m ? parseInt(m[1], 10) === currentId : false;
    });
    if (idx !== -1) active = idx;
  }

  const last = items.length - 1;

  function apply() {
    items.forEach((el, i) => {
      const offset = i - active;
      el.style.setProperty("--offset", offset);
      el.style.setProperty("--abs", Math.abs(offset));
      el.classList.toggle("active", offset === 0);
      el.setAttribute("tabindex", offset === 0 ? "0" : "-1");
      el.setAttribute("aria-hidden", offset !== 0);
    });
  }

  function go(delta) {
    const next = Math.min(last, Math.max(0, active + delta));
    if (next === active) return;
    active = next;
    apply();
  }

  // init positions
  apply();

  // mouse wheel (snappier, accumulated)
  let wheelAccum = 0;
  let lastStep = 0;
  const STEP_THRESHOLD = 30; // smaller = more sensitive
  const STEP_COOLDOWN = 90; // ms between steps

  root.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      const now = performance.now();
      wheelAccum += e.deltaY;

      if (
        Math.abs(wheelAccum) >= STEP_THRESHOLD &&
        now - lastStep > STEP_COOLDOWN
      ) {
        const dir = wheelAccum > 0 ? 1 : -1;
        go(dir);
        lastStep = now;
        wheelAccum = 0;
      }
    },
    { passive: false },
  );
})();
