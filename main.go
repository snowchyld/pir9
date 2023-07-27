package main

import (
	"pir9/models"
	"fmt"
	"log"
	"net/http"
//	"io/ioutil"
//	"strconv"

	"github.com/gin-gonic/gin"
)

func main() {

	err := models.ConnectDatabase()
	checkErr(err)

	r := gin.Default()

	// API v1
	v1 := r.Group("/api/v1")
	{
		v1.GET("ships", getShips)
		v1.POST("ship", addShip)
	}

	// By default it serves on :8080 unless a
	// PORT environment variable was defined.
	r.Run("127.0.0.1:8080")
}

func getShips(c *gin.Context) {

	ships, err := models.GetShips()

	checkErr(err)

	if ships == nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "No Records Found"})
		return
	} else {
		c.JSON(http.StatusOK, gin.H{"data": ships})
	}
}

func addShip(c *gin.Context) {

  // body, _ := ioutil.ReadAll(c.Request.Body)
    //   println(string(body))

	var json models.Ship

	if err := c.ShouldBindJSON(&json); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}
	success, err := models.AddShip(json)
fmt.Print("addShip is here")
	if success {
		c.JSON(http.StatusOK, gin.H{"message": "Success"})
	} else {
		c.JSON(http.StatusBadRequest, gin.H{"error": err})
	}
}

func checkErr(err error) {
	if err != nil {
		log.Fatal(err)
	}
}
