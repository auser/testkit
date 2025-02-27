# Instructions

Generate a comprehensive outline of the project that contains the following functionality for testing with rust and database engines in a test environment:

- Create a new testing database (based off the database engine)
- Run migrations on the testing database
- Expose the functionality to insert data into a table
- Expose the functionality to query data from a table
- Expose the functionality to delete data from a table
- Drop a database after the test is complete and the test pool is dropped for a single test

The current database engines supported are:

- PostgreSQL
- SQLite
- MySQL
- Sqlx Postgres
- Sqlx Mysql
- Sqlx Sqlite

