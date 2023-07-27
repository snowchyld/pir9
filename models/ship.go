package models

import (
	"database/sql"
	//"strconv"
//	"fmt"
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
	hashcode string `"json:hashcode"`
	username string `"json:username"`
	dstIp    string `"json:dstIp"`
	dstPort  string `"json:dstPort"`
}

func GetShips() ([]Ship, error) {

	rows, err := DB.Query("SELECT hashcode, dstIp, dstPort, username from fleet")

	if err != nil {
		return nil, err
	}

	defer rows.Close()

	ships := make([]Ship, 0)

	for rows.Next() {
		singleShip := Ship{}
		err = rows.Scan(&singleShip.hashcode, &singleShip.dstIp, &singleShip.dstPort,  &singleShip.username)

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

	tx, err := DB.Begin()
	if err != nil {
		return false, err
	}

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

	_, err = stmt.Exec(newShip.hashcode, newShip.dstIp, newShip.dstPort, newShip.username)

	if err != nil {
		return false, err
	}

	tx.Commit()

	return true, nil
}
