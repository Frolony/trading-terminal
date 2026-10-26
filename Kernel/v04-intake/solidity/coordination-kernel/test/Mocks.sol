// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;
contract MockConstraint { bool p; constructor(bool p_){p=p_;}
  function validate(bytes32,bytes32,bytes32) external view returns (bool,uint8){ return p ? (true,1) : (false,2);} }
contract MockObjective { function evaluate(bytes32,bytes32,bytes32) external pure returns (int256,bool){ return (10,true);} }
contract MockRelevance { function evaluate(address,bytes32,bytes32) external pure returns (uint256){ return 1e18;} }
contract MockEngine { function evaluate(bytes32, bytes32[] calldata) external pure returns (bool,bytes32[] memory,bytes32[] memory,bytes32[] memory,uint256){
  return (true,new bytes32[](0),new bytes32[](0),new bytes32[](0),1e18);} }
contract MockAttribution { address a; constructor(address a_){a=a_;}
  function evaluate(bytes32,bytes32) external view returns (address[] memory ag,int256[] memory d,bytes32[] memory mp){
   ag=new address[](1); d=new int256[](1); mp=new bytes32[](1); ag[0]=a; d[0]=5e18; mp[0]=keccak256("model-proposal-1"); } }
contract MockPosition {
  mapping(address=>uint256) public amt; mapping(address=>uint256) public until;
  function set(address a,uint256 amount,uint256 lockedUntil) external { amt[a]=amount; until[a]=lockedUntil; }
  function lockedHolding(address a) external view returns (uint256,uint256){ return (amt[a],until[a]); } }
