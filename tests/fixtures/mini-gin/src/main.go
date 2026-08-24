package main

import "github.com/gin-gonic/gin"

func main() {
	r := gin.Default()
	r.POST("/sms", store)
}

func store(c *gin.Context) {
	SmsStore{}.Save(c.PostForm("body"))
}
