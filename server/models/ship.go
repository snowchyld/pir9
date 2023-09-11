package models

import (
	"database/sql"
	//"strconv"
	"fmt"
		"encoding/json"
	_ "github.com/mattn/go-sqlite3"
)

var DB *sql.DB

func ConnectDatabase() error {
	db, err := sql.Open("sqlite3", "./db/fleet.db")
	if err != nil {
		return err
	}

	DB = db
	return nil
}

type Ship struct {
	Hashcode string `json:"hashcode"`
	DstIp    string `json:"dstIp"`
	DstPort  string `json:"dstPort"`
	Username string `json:"username"`
}

type Fleet struct {
	DstIp    string `json:"dstIp"`
	DstPort  string `json:"dstPort"`
	Username string `json:"username"`
}

func GetShips() ([]Fleet, error) {

	rows, err := DB.Query("SELECT dstIp, dstPort, username from fleet")

	if err != nil {
		return nil, err
	}

	defer rows.Close()

	ships := make([]Fleet, 0)

	for rows.Next() {
		singleShip := Fleet{}
		err = rows.Scan(&singleShip.DstIp, &singleShip.DstPort, &singleShip.Username)

		if err != nil {
			return nil, err
		}

		ships = append(ships, singleShip)
	}

	err = rows.Err()

	if err != nil {
		return nil, err
	}

	return ships, err
}

func AddShip(newShip Ship) (bool, error) {

		j, _ := json.MarshalIndent(newShip, "", "  "); fmt.Println(string(j))

	//	fmt.Println("vals=" + newShip.Hashcode + " - " + newShip.DstIp + " - " + newShip.DstPort + " - " + newShip.Username)
	tx, err := DB.Begin()
	if err != nil {
		return false, err
	}

	fmt.Println("val=" + newShip.Username)
	/*
	  stmt, err := db.Prepare("INSERT OR REPLACE INTO fleet (hashcode, dstIp, dstPort, username) VALUES (?, ?, ?, ?)")
	          //      Console.Writeln(stmt);
		          //      fmt.Printf(stmt)
			          stmt.Exec(newShip.hashcode, newShip.dstIp, newShip.dstPort, newShip.username)
				          stmt.Close()

	*/
	stmt, err := tx.Prepare("INSERT OR REPLACE INTO fleet (hashcode, dstIp, dstPort, username) VALUES (?, ?, ?, ?)")

	if err != nil {
		return false, err
	}

	defer stmt.Close()

	_, err = stmt.Exec(newShip.Hashcode, newShip.DstIp, newShip.DstPort, newShip.Username)

	if err != nil {
		return false, err
	}

	tx.Commit()

	return true, nil
}
