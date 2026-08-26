export function login(userInfo) {
  return request({ url: "/user/login", method: "post", data: userInfo });
}

export function getInfo() {
  return request({ url: "/user/info", method: "get" });
}

export function logout() {
  return request({ url: "/user/logout", method: "post" });
}
