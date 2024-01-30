create database db;
create user manager@'%' identified by 'whalehello';
grant all on db.* to manager;
